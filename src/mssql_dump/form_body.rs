use super::*;
use crate::form_schema::{
    FORM_DECORATION_HEADER_XML_ORDER, FORM_EXTENDED_TOOLTIP_XML_ORDER,
    FORM_FIELD_HEADER_PICTURE_XML_ORDER, FORM_INPUT_FIELD_BUTTON_XML_ORDER,
    FORM_INPUT_FIELD_TAIL_XML_ORDER, FORM_LABEL_DECORATION_ALIGNMENT_TAIL_XML_ORDER,
    FORM_LABEL_DECORATION_GEOMETRY_XML_ORDER, FORM_LABEL_DECORATION_VISUAL_TAIL_XML_ORDER,
    FORM_MOBILE_DEVICE_COMMAND_BAR_CONTENT_ITEM_XML_ORDER, FORM_PAGE_XML_ORDER,
    FORM_PICTURE_DECORATION_GEOMETRY_XML_ORDER, FORM_TABLE_XML_ORDER,
    FORM_USUAL_GROUP_HEADER_XML_ORDER, FORM_USUAL_GROUP_XML_ORDER,
    FormAttributeAdditionalColumnsBindingKind, FormAttributeAdditionalColumnsGroupSchema,
    FormAttributeColumnSchema, FormButtonColorSchema, FormButtonCommonSchema,
    FormButtonShapeRepresentationSchema, FormCheckBoxFieldSchema, FormChildItemAlignment,
    FormChildItemDisplayImportanceSchema, FormChildItemEventCollectionSchema,
    FormChildItemShowTitleSchema, FormChildItemUserVisibleSchema, FormChildItemVisibleSchema,
    FormCommandBarSchema, FormCommandCurrentRowUse, FormCommandInterfaceContainerOwner,
    FormCommandInterfaceContainerSchema, FormCommandInterfaceItemSchema,
    FormCommandInterfaceVisibilitySchema, FormCommandSchema, FormConditionalGroupSchema,
    FormConditionalTableSchema, FormContainerReadOnlySchema, FormDecorationHeaderSchema,
    FormDecorationHeaderXmlProperty, FormExtendedTooltipSchema, FormExtendedTooltipXmlProperty,
    FormFieldHeaderPictureSchema, FormFieldHeaderPictureXmlProperty, FormFieldSchema,
    FormFieldTitleLocationSchema, FormFieldTopLevelSlot as FieldSlot,
    FormInputFieldExtendedOptionSlot as InputFieldSlot, FormInputFieldTailXmlProperty,
    FormInputFieldXmlProperty, FormLabelDecorationAlignment,
    FormLabelDecorationAlignmentTailXmlProperty, FormLabelDecorationGeometry,
    FormLabelDecorationGeometryXmlProperty, FormLabelDecorationSchema,
    FormLabelDecorationVisualTail, FormLabelDecorationVisualTailXmlProperty,
    FormLabelFieldOptionSlot as LabelFieldSlot, FormMobileDeviceCommandBarContentItemXmlProperty,
    FormNestedAutoCommandBarSchema, FormPageSchema, FormPageXmlProperty,
    FormPictureDecorationGeometryXmlProperty, FormPictureDecorationSchema, FormPictureValueKind,
    FormPopupSchema, FormRootAutoUrlSchema, FormRootGroupSchema,
    FormRootMobileDeviceCommandBarContentSchema, FormRootVerticalScrollSchema,
    FormSharedContainerContentChangeSchema, FormSpecialFieldSchema,
    FormSpreadsheetDocumentFieldProperties, FormTableOrdinaryTailKey as TableTailKey,
    FormTablePropertyBagKey as TableBagKey, FormTableRootPropertyBagKey as TableRootBagKey,
    FormTableRowPictureDataPath, FormTableSchema, FormTableSearchControlLocation,
    FormTableSearchStringLocation, FormTableViewStatusLocation, FormTableXmlProperty,
    FormTooltipRepresentationXmlOrder, FormUsualGroupGroupVerticalAlign,
    FormUsualGroupHeaderXmlProperty, FormUsualGroupSchema, FormUsualGroupXmlAnchor,
    FormUsualGroupXmlProperty, decode_form_tooltip_representation,
    form_attribute_column_builtin_type_reference, form_child_item_representation_is_default,
    form_tooltip_representation_schema, form_tooltip_representation_xml_order,
};
use uuid::Uuid;

const FORM_STANDARD_DATA_PATH_NAME_ALIASES: &[(&str, &str)] = &[
    ("Ссылка", "Ref"),
    ("ПометкаУдаления", "DeletionMark"),
    ("ЭтоГруппа", "IsFolder"),
    ("Владелец", "Owner"),
    ("Родитель", "Parent"),
    ("Наименование", "Description"),
    ("Код", "Code"),
    ("Предопределенный", "Predefined"),
    ("ИмяПредопределенныхДанных", "PredefinedDataName"),
    ("Проведен", "Posted"),
    ("Дата", "Date"),
    ("Номер", "Number"),
    ("Период", "Period"),
    ("Регистратор", "Recorder"),
    ("НомерСтроки", "LineNumber"),
    ("Активность", "Active"),
    ("ВидДвижения", "RecordType"),
    ("ТипЗначения", "ValueType"),
];

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
    extract_form_body_xml_from_body_timed(
        body,
        type_index,
        object_refs,
        &BTreeMap::new(),
        None,
        None,
    )
}

pub(super) fn extract_form_body_xml_from_body_timed(
    body: &ParsedFormBodyBlob,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    information_register_field_refs: &InformationRegisterFieldReferenceIndex,
    form_owner_reference: Option<&str>,
    mut timings: Option<&mut MssqlDumpTimingReport>,
) -> Option<String> {
    let started = Instant::now();
    let form_fields = split_1c_braced_fields(&body.layout, 0)?;
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_split_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let mut properties = extract_form_body_properties(&form_fields, form_owner_reference);
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
    let child_item_indexes =
        collect_form_child_item_indexes_with_object_refs(&form_fields, &attributes, object_refs);
    properties.mobile_device_command_bar_content = extract_form_mobile_device_command_bar_content(
        &form_fields,
        &child_item_indexes.item_name_by_id,
    );
    let child_item_indexes_cpu_ms = elapsed_ms(started);

    let started = Instant::now();
    let commands = extract_form_body_commands(
        &body.trailing,
        object_refs,
        &child_item_indexes.item_name_by_id,
    );
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_commands_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let auto_command_bar = extract_form_auto_command_bar(
        &form_fields,
        &commands,
        object_refs,
        &child_item_indexes.table_name_by_id,
        &child_item_indexes.standard_command_owner_name_by_id,
        &child_item_indexes.command_source_owner_name_by_id,
    );
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_auto_command_bar_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
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
        object_refs,
    );
    let child_items = extract_form_child_items(
        &form_fields,
        &attributes,
        &commands,
        object_refs,
        &child_item_indexes,
    );
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_child_items_cpu_ms +=
            child_item_indexes_cpu_ms + elapsed_ms(started);
    }

    let started = Instant::now();
    let command_interface = extract_form_command_interface_with_context(
        &body.trailing,
        &commands,
        object_refs,
        information_register_field_refs,
        form_owner_reference,
        &attributes,
        &child_item_indexes,
    );
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
    pub(super) mobile_device_command_bar_content: Vec<String>,
    pub(super) show_title: Option<bool>,
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
pub(super) const FORM_ITEM_TYPE_UUID: &str = "02023637-7868-4a5f-8576-835a76e0c9ba";
pub(super) const FORM_GLOBAL_COMMAND_SOURCE_TYPE_UUID: &str =
    "2ef6d6fa-847a-485e-8684-d37a3ab5efb8";
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
    pub(super) display_importance: Option<&'static str>,
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
    pub(super) exact_single_type_uuid: Option<String>,
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
pub(super) struct FormAttributeMetadataOwner {
    name: String,
    type_references: Vec<String>,
    exact_single_type_reference: Option<String>,
    has_dynamic_list_settings: bool,
    main_table: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormAttributeSaveFieldBinding {
    pub(super) key: String,
    pub(super) metadata_uuid: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum FormAttributeSaveEntry {
    SelfValue,
    Binding(FormAttributeSaveFieldBinding),
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
    pub(super) manual_query_explicit: bool,
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
    pub(super) current_row_use: Option<FormCommandCurrentRowProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormCommandCurrentRowProperties {
    pub(super) value: Option<FormCommandCurrentRowUse>,
    pub(super) associated_table_element_id: Option<String>,
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
    pub(super) attribute: Option<String>,
    pub(super) command_group: Option<String>,
    pub(super) index: Option<usize>,
    pub(super) default_visible: Option<bool>,
    pub(super) visible: Option<FormCommandInterfaceVisibility>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormCommandInterfaceVisibility {
    pub(super) common: bool,
    pub(super) role_values: Vec<(String, bool)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormExtendedTooltipTitle {
    pub(super) values: Vec<(String, String)>,
    pub(super) formatted: bool,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct FormExtendedTooltip {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) width: Option<String>,
    pub(super) auto_max_width: Option<bool>,
    pub(super) max_width: Option<String>,
    pub(super) height: Option<String>,
    pub(super) auto_max_height: Option<bool>,
    pub(super) horizontal_stretch: Option<bool>,
    pub(super) vertical_stretch: Option<bool>,
    pub(super) text_color: Option<String>,
    pub(super) font_xml: Option<String>,
    pub(super) title: Option<FormExtendedTooltipTitle>,
    pub(super) group_horizontal_align: Option<&'static str>,
    pub(super) vertical_align: Option<&'static str>,
    pub(super) events: Vec<FormBodyEvent>,
}

impl FormExtendedTooltip {
    pub(super) fn new(name: String, id: String) -> Self {
        Self {
            id,
            name,
            ..Self::default()
        }
    }

    fn has_properties(&self) -> bool {
        self.width.is_some()
            || self.auto_max_width.is_some()
            || self.max_width.is_some()
            || self.height.is_some()
            || self.auto_max_height.is_some()
            || self.horizontal_stretch.is_some()
            || self.vertical_stretch.is_some()
            || self.text_color.is_some()
            || self.font_xml.is_some()
            || self.title.is_some()
            || self.group_horizontal_align.is_some()
            || self.vertical_align.is_some()
            || !self.events.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum FormChildItemDataPathProvenance {
    DirectRawSlot,
    InferredFallback,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormChildItem {
    pub(super) tag: &'static str,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) display_importance: Option<&'static str>,
    pub(super) auto_command_bar_empty_element: bool,
    pub(super) autofill: Option<bool>,
    pub(super) group: Option<&'static str>,
    pub(super) behavior: Option<&'static str>,
    pub(super) representation: Option<&'static str>,
    pub(super) usual_group_enabled: Option<bool>,
    pub(super) enable_content_change: Option<bool>,
    pub(super) child_items_width: Option<&'static str>,
    pub(super) control_representation: Option<&'static str>,
    pub(super) collapsed: Option<bool>,
    pub(super) usual_group_collapsed_representation_title: Vec<(String, String)>,
    pub(super) usual_group_children_align: Option<&'static str>,
    pub(super) usual_group_horizontal_spacing: Option<&'static str>,
    pub(super) usual_group_vertical_spacing: Option<&'static str>,
    pub(super) usual_group_horizontal_align: Option<&'static str>,
    pub(super) usual_group_vertical_align: Option<&'static str>,
    pub(super) usual_group_group_vertical_align: Option<FormUsualGroupGroupVerticalAlign>,
    pub(super) through_align: Option<&'static str>,
    pub(super) united: Option<bool>,
    pub(super) usual_group_show_left_margin: Option<bool>,
    pub(super) table_representation: Option<&'static str>,
    pub(super) table_command_bar_location: Option<&'static str>,
    pub(super) table_search_string_location: Option<FormTableSearchStringLocation>,
    pub(super) table_view_status_location: Option<FormTableViewStatusLocation>,
    pub(super) table_search_control_location: Option<FormTableSearchControlLocation>,
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
    pub(super) initial_tree_view: Option<&'static str>,
    pub(super) row_input_mode: Option<&'static str>,
    pub(super) table_choice_mode: Option<bool>,
    pub(super) table_selection_mode: Option<&'static str>,
    pub(super) table_header: Option<bool>,
    pub(super) table_horizontal_lines: Option<bool>,
    pub(super) table_vertical_lines: Option<bool>,
    pub(super) show_root: Option<bool>,
    pub(super) allow_root_choice: Option<bool>,
    pub(super) choice_folders_and_items: Option<&'static str>,
    pub(super) restore_current_row: Option<bool>,
    pub(super) row_filter_nil: Option<bool>,
    pub(super) row_picture_data_path: Option<String>,
    pub(super) rows_picture_ref: Option<String>,
    pub(super) rows_picture_file_name: Option<String>,
    pub(super) rows_picture_load_transparent: bool,
    pub(super) top_level_parent_nil: Option<bool>,
    pub(super) update_on_data_change: Option<&'static str>,
    pub(super) user_settings_group: Option<String>,
    pub(super) allow_getting_current_row_url: Option<bool>,
    pub(super) button_representation: Option<&'static str>,
    pub(super) shape_representation: Option<&'static str>,
    pub(super) representation_in_context_menu: Option<&'static str>,
    pub(super) group_horizontal_align: Option<&'static str>,
    pub(super) horizontal_location: Option<&'static str>,
    pub(super) location_in_command_bar: Option<&'static str>,
    pub(super) default_button: Option<bool>,
    pub(super) scroll_on_compress: Option<bool>,
    pub(super) show_title: Option<bool>,
    pub(super) show_in_header: Option<bool>,
    pub(super) user_visible_common: Option<bool>,
    pub(super) visible: Option<bool>,
    pub(super) enabled: Option<bool>,
    pub(super) read_only: Option<bool>,
    pub(super) skip_on_input: Option<bool>,
    pub(super) strict_table_schema: bool,
    pub(super) title_location: Option<&'static str>,
    pub(super) title_height: Option<String>,
    pub(super) tooltip_representation: Option<&'static str>,
    pub(super) edit_mode: Option<&'static str>,
    pub(super) horizontal_align: Option<FormChildItemAlignment>,
    pub(super) group_vertical_align: Option<&'static str>,
    pub(super) label_decoration_visual_tail: Option<FormLabelDecorationVisualTail>,
    pub(super) check_box_type: Option<&'static str>,
    pub(super) radio_button_type: Option<&'static str>,
    pub(super) columns_count: Option<u32>,
    pub(super) cell_hyperlink: Option<bool>,
    pub(super) show_in_footer: Option<bool>,
    pub(super) footer_horizontal_align: Option<&'static str>,
    pub(super) hiperlink: Option<bool>,
    pub(super) text_color: Option<String>,
    pub(super) back_color: Option<String>,
    pub(super) border_color: Option<String>,
    pub(super) title_text_color: Option<String>,
    pub(super) mark_required_complete: Option<bool>,
    pub(super) auto_edit_mode: Option<bool>,
    pub(super) auto_insert_new_row: Option<bool>,
    pub(super) format: Vec<(String, String)>,
    pub(super) edit_format: Vec<(String, String)>,
    pub(super) title_font_xml: Option<String>,
    pub(super) font_xml: Option<String>,
    pub(super) width: Option<String>,
    pub(super) height: Option<String>,
    pub(super) show_current_date: Option<bool>,
    pub(super) show_months_panel: Option<bool>,
    pub(super) width_in_months: Option<String>,
    pub(super) height_in_months: Option<String>,
    pub(super) auto_max_width: Option<bool>,
    pub(super) max_width: Option<String>,
    pub(super) auto_max_height: Option<bool>,
    pub(super) max_height: Option<String>,
    pub(super) horizontal_stretch: Option<bool>,
    pub(super) vertical_stretch: Option<bool>,
    pub(super) spreadsheet_document_properties: Option<FormSpreadsheetDocumentFieldProperties>,
    pub(super) max_value: Option<String>,
    pub(super) input_min_value: Option<String>,
    pub(super) input_max_value: Option<String>,
    pub(super) show_percent: Option<bool>,
    pub(super) password_mode: Option<bool>,
    pub(super) multi_line: Option<bool>,
    pub(super) wrap: Option<bool>,
    pub(super) extended_edit: Option<bool>,
    pub(super) mask: Option<String>,
    pub(super) text_edit: Option<bool>,
    pub(super) edit_text_update: Option<&'static str>,
    pub(super) auto_cell_height: Option<bool>,
    pub(super) drop_list_button: Option<bool>,
    pub(super) clear_button: Option<bool>,
    pub(super) open_button: Option<bool>,
    pub(super) create_button: Option<bool>,
    pub(super) choice_button: Option<bool>,
    pub(super) choice_list_button: Option<bool>,
    pub(super) spin_button: Option<bool>,
    pub(super) list_choice_mode: Option<bool>,
    pub(super) extended_edit_multiple_values: Option<bool>,
    pub(super) quick_choice: Option<bool>,
    pub(super) choose_type: Option<bool>,
    pub(super) auto_choice_incomplete: Option<bool>,
    pub(super) auto_mark_incomplete: Option<bool>,
    pub(super) incomplete_choice_mode: Option<&'static str>,
    pub(super) choice_button_representation: Option<&'static str>,
    pub(super) choice_button_picture_ref: Option<String>,
    pub(super) choice_button_picture_load_transparent: bool,
    pub(super) drop_list_width: Option<String>,
    pub(super) choice_history_on_input: Option<&'static str>,
    pub(super) item_type: Option<&'static str>,
    pub(super) addition_source_item: Option<String>,
    pub(super) picture_ref: Option<String>,
    pub(super) picture_load_transparent: bool,
    pub(super) header_picture_ref: Option<String>,
    pub(super) header_picture_file_name: Option<String>,
    pub(super) header_picture_load_transparent: bool,
    pub(super) picture_size: Option<&'static str>,
    pub(super) picture_file_name: Option<&'static str>,
    pub(super) title: Vec<(String, String)>,
    pub(super) usual_group_shortcut: Option<String>,
    pub(super) title_formatted: Option<bool>,
    pub(super) tooltip: Vec<(String, String)>,
    pub(super) input_hint: Vec<(String, String)>,
    pub(super) choice_list: Vec<FormChoiceListItem>,
    pub(super) choice_parameter_links: Vec<FormChoiceParameterLink>,
    pub(super) type_link: Option<FormTypeLink>,
    pub(super) extended_tooltip: Option<FormExtendedTooltip>,
    pub(super) events: Vec<FormBodyEvent>,
    pub(super) data_path: Option<String>,
    pub(super) data_path_provenance: Option<FormChildItemDataPathProvenance>,
    pub(super) title_data_path: Option<String>,
    pub(super) command_name: Option<String>,
    pub(super) command_source: Option<String>,
    pub(super) child_items: Vec<FormChildItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormChoiceListItem {
    pub(super) presentation_present: bool,
    pub(super) presentation: Vec<(String, String)>,
    pub(super) value: FormChoiceListValue,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum FormChoiceListValue {
    Boolean(bool),
    Decimal(String),
    Nil,
    String(String),
    DesignTimeRef(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormChoiceParameterLink {
    pub(super) name: String,
    pub(super) data_path: String,
    pub(super) value_change: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormTypeLink {
    pub(super) data_path: String,
    pub(super) link_item: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormTablePeriod {
    pub(super) variant: &'static str,
    pub(super) start_date: String,
    pub(super) end_date: String,
}

pub(super) fn extract_form_body_properties(
    fields: &[&str],
    form_owner_reference: Option<&str>,
) -> FormBodyProperties {
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
        command_set_excluded_commands: extract_form_command_set_excluded_commands(
            fields,
            form_owner_reference,
        ),
        use_for_folders_and_items: extract_form_use_for_folders_and_items(fields),
        customizable: extract_form_customizable(fields),
        command_bar_location: extract_form_command_bar_location(fields),
        vertical_scroll: extract_form_vertical_scroll(fields),
        horizontal_align: extract_form_horizontal_align(fields),
        conversations_representation: extract_form_conversations_representation(fields),
        mobile_device_command_bar_content: Vec::new(),
        show_title: extract_form_show_title(fields),
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
    let root_discriminator = fields.first().map(|field| field.trim());
    match root_discriminator {
        Some("50") => {
            let tail_start = form_root_child_items_tail_start(fields)?;
            FormRootAutoUrlSchema::from_raw_layout(root_discriminator, fields.get(tail_start..)?)?
                .auto_url()
        }
        Some("59") => FormRootAutoUrlSchema::from_legacy_raw_layout(
            root_discriminator,
            fields,
            form_root_uses_property_bag(fields),
        )?
        .auto_url(),
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
    let root_discriminator = fields.first().map(|field| field.trim());
    match root_discriminator {
        Some("50") => {
            let tail_start = form_root_child_items_tail_start(fields)?;
            FormRootGroupSchema::from_raw_layout(
                root_discriminator,
                fields.get(11).copied(),
                fields.get(tail_start..)?,
            )?
            .group()
        }
        Some("59") => {
            FormRootGroupSchema::from_legacy_raw_layout(root_discriminator, fields)?.group()
        }
        _ => None,
    }
}

pub(super) fn extract_form_command_set_excluded_commands(
    fields: &[&str],
    form_owner_reference: Option<&str>,
) -> Vec<&'static str> {
    let Some(property_count) = fields
        .get(18)
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    let Some(command_slot) = property_count
        .checked_mul(2)
        .and_then(|offset| 20usize.checked_add(offset))
    else {
        return Vec::new();
    };
    let Some(command_set) = fields
        .get(command_slot)
        .and_then(|field| parse_form_table_counted_uuid_list(field))
    else {
        return Vec::new();
    };
    let business_process_or_task = form_owner_reference.is_some_and(|owner| {
        matches!(
            owner.split_once('.').map(|(kind, _)| kind),
            Some("BusinessProcess" | "Task")
        )
    });
    let Some(mut commands) = command_set
        .iter()
        .map(|uuid| form_standard_excluded_command_name(uuid, business_process_or_task))
        .collect::<Option<Vec<_>>>()
    else {
        return Vec::new();
    };
    commands.sort_unstable();
    commands
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
    if fields.first().map(|field| field.trim()) == Some("50") {
        return (fields.get(13).map(|field| field.trim()) == Some("0")).then_some(false);
    }
    if fields.first().map(|field| field.trim()) != Some("59") {
        return None;
    }
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
    let trailer = fields.get(tail_start..)?;
    FormRootVerticalScrollSchema::from_raw_layout(
        fields.first().map(|field| field.trim()),
        trailer.len(),
    )?
    .vertical_scroll(trailer)
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

pub(super) fn extract_form_show_close_button(fields: &[&str]) -> Option<bool> {
    let tail_start = form_root_child_items_tail_start(fields)?;
    match fields.get(tail_start + 18).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_mobile_device_command_bar_content(
    fields: &[&str],
    item_name_by_id: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(tail_start) = form_root_child_items_tail_start(fields) else {
        return Vec::new();
    };
    let Some(trailer) = fields.get(tail_start..) else {
        return Vec::new();
    };
    let Some(content) = trailer
        .get(FormRootMobileDeviceCommandBarContentSchema::CONTENT_TRAILER_SLOT)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    else {
        return Vec::new();
    };
    let Some(declared_item_count) = content
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    let parsed_ids = content
        .get(2..)
        .unwrap_or_default()
        .chunks_exact(2)
        .filter_map(|pair| {
            if pair.first()?.trim() != "\"\"" {
                return None;
            }
            let value = split_1c_braced_fields(pair.get(1)?.trim(), 0)?;
            if value.len() != 2 || value.first()?.trim() != "\"N\"" {
                return None;
            }
            let id = value.get(1)?.trim();
            id.parse::<u64>().ok().map(|_| id.to_string())
        })
        .collect::<Vec<_>>();
    let Some(schema) = FormRootMobileDeviceCommandBarContentSchema::from_raw_layout(
        fields.first().map(|field| field.trim()),
        trailer.len(),
        content.first().map(|field| field.trim()),
        content.len(),
        declared_item_count,
        parsed_ids.len(),
    ) else {
        return Vec::new();
    };
    if schema.item_count() != parsed_ids.len() {
        return Vec::new();
    }
    let mut items = Vec::with_capacity(schema.item_count());
    for id in parsed_ids {
        if id == "0" {
            items.push(String::new());
        } else if let Some(name) = item_name_by_id.get(&id) {
            items.push(name.clone());
        } else {
            return Vec::new();
        }
    }
    items
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

pub(super) fn form_standard_excluded_command_name(
    uuid: &str,
    business_process_or_task: bool,
) -> Option<&'static str> {
    if business_process_or_task && uuid == "32df4349-2607-4c2b-a4b9-bca4a1a28bd7" {
        return Some("ExecuteAndClose");
    }
    match uuid {
        "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d" => Some("No"),
        "0b83270d-7f95-4cdd-93c3-342d7991fed5" => Some("Tree"),
        "0ea1a92b-3477-44dd-b152-ea7d411f1c5d" => Some("OpenFromMainServer"),
        "0fb774df-ec1c-4e23-9ed1-e089974f74bf" => Some("ReportSettings"),
        "174e58ce-82ad-4787-b956-9367937f7971" => Some("ChangeHistory"),
        "198ea630-fda2-4cda-8a23-f999f4c67ee6" => Some("CustomizeForm"),
        "1c00edb8-a826-4855-9bde-94dbc5f620e5" => Some("ListSettings"),
        "1cc781aa-f32b-4dc7-996a-6c38c3deda5c" => Some("Delete"),
        "1f317795-c420-4a30-b594-c492abc55f7a" => Some("Reread"),
        "239f0103-8de9-4fdf-b485-eb5531da7e51" => Some("SaveValues"),
        "2cacadf7-8fb3-4ec6-ae2b-0ca3fd311c9e" => Some("Execute"),
        "2e86453d-8958-4c9a-a1b4-b15215eedc2e" => Some("SetDeletionMark"),
        "32df4349-2607-4c2b-a4b9-bca4a1a28bd7" => Some("WriteAndClose"),
        "3328a951-c3c8-4f22-b99e-814f7cea6b82" => Some("ReadChanges"),
        "342c531d-dc73-458a-8ac4-6a746916a33b" => Some("Copy"),
        "3772996b-41f4-4c47-a5a8-ea397db424ae" => Some("Close"),
        "389ef1f1-97ce-4326-adf5-886b2dead75c" => Some("UndoPosting"),
        "39bb0fe9-771d-4dd5-8a6e-2d16984523af" => Some("Help"),
        "39c6a2fb-45cc-41b1-853f-967fb68aa1df" => Some("MoveItem"),
        "3a17e914-ec6a-4280-b4df-78914f40522b" => Some("ShowInList"),
        "3b8cedbc-8e74-4017-b901-d14b09f32f7a" => Some("Post"),
        "3dd3bd8a-ac1e-44d6-ac83-e7802642a5e2" => Some("Delete"),
        "3ea8bf45-5f33-4545-a3bb-29f80666b627" => Some("ChangeSettingsStructure"),
        "3f01ed62-97f8-465b-b4f7-6517ac2bc994" => Some("Abort"),
        "4f834c38-add1-45e4-a9f3-cefe3efac5c9" => Some("Create"),
        "5174ad3f-0569-42fd-8adf-011d8206db6c" => Some("Retry"),
        "573e81b7-57eb-45f0-ba4d-ada7c2537a2d" => Some("OpenFromStandaloneServer"),
        "5d41082e-9619-42ec-b96f-98b082b3a2f0" => Some("Yes"),
        "679b62d9-ff72-4329-bf3a-c0c32b311dd2" => Some("Cancel"),
        "6886601d-276c-4d3f-af0a-05c586025608" => Some("Change"),
        "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988" => Some("Copy"),
        "6f959e83-23ec-4991-901d-575d7ea98868" => Some("Activate"),
        "71e0226e-ebb2-4e33-8745-0a94a01bbf15" => Some("RestoreValues"),
        "74c1abd6-b274-4654-baf0-7b8418b792ea" => Some("EndEdit"),
        "7910bb04-ddcc-4e5d-89f0-104c6ad0f187" => Some("SaveReportSettings"),
        "8149a06a-dbf3-4d4d-a275-5385a4196fc7" => Some("CancelEdit"),
        "827b541d-30c1-4f06-aecf-92aa496a0835" => Some("SetDeletionMark"),
        "87317f86-057f-477e-9045-2da4e4980199" => Some("PostAndClose"),
        "8b81add7-25af-4df7-a69c-144e3e3e4c8e" => Some("WriteChanges"),
        "952c2984-9955-415a-8235-5c710aabe732" => Some("LoadDynamicListSettings"),
        "96e0bc70-f8ff-4732-8119-060923203629" => Some("CancelSearch"),
        "9758d344-4b1d-4dc9-80bd-81060bc18b2a" => Some("OutputList"),
        "9bffcf73-7b1d-4a8d-bf23-5e051af3ee29" => Some("SaveVariant"),
        "9fea4ba9-7d33-47d4-a271-cb54df4a9b74" => Some("ShowMultipleSelection"),
        "a29c4f3a-3b41-480a-a31e-5f9f73aa3216" => Some("WriteChanges"),
        "a2b927a1-35af-43e3-af73-4af22ac2c0fa" => Some("List"),
        "aa042316-63ba-4f10-8d39-3935474562d0" => Some("LevelDown"),
        "b08b7a35-583a-4756-b814-0436ff9139c0" => Some("LoadVariant"),
        "b0c9afb6-320c-4e36-be21-8f6d48116415" => Some("LoadReportSettings"),
        "b520ca45-d8db-4982-b128-bb42a6afd911" => Some("FindByCurrentValue"),
        "b5e6da6b-cec4-450c-876a-6a5f0837f6cc" => Some("Generate"),
        "bdefa701-6685-453e-a02a-3683d0cc16d3" => Some("Find"),
        "c32d43de-b820-49d0-bf7a-d70829f48f40" => Some("Delete"),
        "c8f1bd8c-b4d1-46d5-97b3-929b5606b6c3" => Some("StandardSettings"),
        "c9abb6b0-eafd-4505-8312-9a7b6888cbf3" => Some("ChangeHistory"),
        "d5c3842d-7252-4370-9174-756a6cc553e5" => Some("SaveDynamicListSettings"),
        "d603a249-6eb3-4e38-bb2d-a8a86a8ab156" => Some("DynamicListStandardSettings"),
        "d7e9e72c-8fa7-430c-a3e9-aeadfd57dfc7" => Some("Ignore"),
        "d82e191e-f052-40ee-8691-00cac5b34629" => Some("CreateInitialImage"),
        "d8772fd1-a3bf-417d-8334-c49968dbb45e" => Some("CreateFolder"),
        "e44f9b41-bf53-4837-b4d4-f0ff9cdf0feb" => Some("LevelUp"),
        "e7ae2a27-60a2-44ae-ab1d-f307d11c85bf" => Some("ReadChanges"),
        "f3613d5c-20c6-46e5-b4d5-7d712ece1296" => Some("OK"),
        "f4613f71-5449-48ed-aea5-de005b272a1d" => Some("SwitchActivity"),
        "fb9d7977-258a-440a-9b59-0a650c86f6a2" => Some("ChangeVariant"),
        "fd8f031f-c168-4e1b-8b0c-15eb3057e688" => Some("Refresh"),
        "fe558fde-99b3-45d0-a060-9fc2905309f6" => Some("Write"),
        "ffc5e8d5-40a7-4893-a590-49bd588f9466" => Some("HierarchicalList"),
        _ => None,
    }
}

pub(super) fn extract_form_auto_command_bar(
    fields: &[&str],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    command_source_owner_name_by_id: &BTreeMap<String, String>,
) -> Option<FormAutoCommandBar> {
    find_form_auto_command_bar(
        fields,
        commands,
        object_refs,
        table_name_by_id,
        standard_command_owner_name_by_id,
        command_source_owner_name_by_id,
    )
}

pub(super) fn find_form_auto_command_bar(
    fields: &[&str],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    command_source_owner_name_by_id: &BTreeMap<String, String>,
) -> Option<FormAutoCommandBar> {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if let Some(command_bar) = parse_form_auto_command_bar_fields(
            &nested,
            commands,
            object_refs,
            table_name_by_id,
            standard_command_owner_name_by_id,
            command_source_owner_name_by_id,
        ) {
            return Some(command_bar);
        }
        if let Some(command_bar) = find_form_auto_command_bar(
            &nested,
            commands,
            object_refs,
            table_name_by_id,
            standard_command_owner_name_by_id,
            command_source_owner_name_by_id,
        ) {
            return Some(command_bar);
        }
    }
    None
}

pub(super) fn parse_form_auto_command_bar_fields(
    fields: &[&str],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    command_source_owner_name_by_id: &BTreeMap<String, String>,
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
        display_importance: FormChildItemDisplayImportanceSchema::from_raw_layout(
            "22",
            fields.len(),
            "AutoCommandBar",
            0,
        )
        .and_then(|schema| schema.display_importance(fields)),
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
            table_name_by_id,
            standard_command_owner_name_by_id,
            command_source_owner_name_by_id,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &FormOwnerScopedBindingIndexes::default(),
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

fn is_raw_empty_nested_auto_command_bar(
    wrapper: &str,
    tag: &str,
    id: &str,
    fields: &[&str],
) -> bool {
    if wrapper != "22" || tag != "AutoCommandBar" || id == "-1" || fields.len() != 29 {
        return false;
    }
    let Some(marker_text) = fields.get(20).map(|field| field.trim()) else {
        return false;
    };
    if scan_1c_braced_value(marker_text, 0) != Some(marker_text.len()) {
        return false;
    }
    let Some(marker) = split_1c_braced_fields(marker_text, 0) else {
        return false;
    };
    marker.len() == 3 && marker.iter().map(|field| field.trim()).eq(["0", "0", "1"])
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
        "ActivationProcessing" => Some("ActivationProcessing"),
        "AdditionalDetailProcessing" => Some("AdditionalDetailProcessing"),
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
        "BeforeCollapse" => Some("BeforeCollapse"),
        "BeforeEditEnd" => Some("BeforeEditEnd"),
        "BeforeExpand" => Some("BeforeExpand"),
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
        "Click" => Some("Click"),
        "Creating" => Some("Creating"),
        "DetailProcessing" => Some("DetailProcessing"),
        "DocumentComplete" => Some("DocumentComplete"),
        "EditTextChange" => Some("EditTextChange"),
        "NotificationProcessing" => Some("NotificationProcessing"),
        "ExternalEvent" => Some("ExternalEvent"),
        "NavigationProcessing" => Some("NavigationProcessing"),
        "NewWriteProcessing" => Some("NewWriteProcessing"),
        "OnActivateField" => Some("OnActivateField"),
        "Selection" => Some("Selection"),
        "StartListChoice" => Some("StartListChoice"),
        "TextEditEnd" => Some("TextEditEnd"),
        "URLGetProcessing" => Some("URLGetProcessing"),
        "URLProcessing" => Some("URLProcessing"),
        "Opening" => Some("Opening"),
        "OnReopen" => Some("OnReopen"),
        "OnActivate" => Some("OnActivate"),
        "OnMainServerAvailabilityChange" => Some("OnMainServerAvailabilityChange"),
        "047d4d09-961c-4bdc-8519-eef10674c35b" => Some("AfterWrite"),
        "0b8dc702-d001-4637-a215-9f35613e096c" => Some("AdditionalDetailProcessing"),
        "14256303-d2b7-4a58-bfab-e77493d10a59" => Some("EditTextChange"),
        "1dd89674-8b50-4240-9899-e3426b79cb02" => Some("BeforeLoadVariantAtServer"),
        "2042ec93-3108-4190-b767-ec6c10dd9ff4" => Some("ActivationProcessing"),
        "213d1900-dcad-4616-9f20-3f077156a40f" => Some("AfterWriteAtServer"),
        "22287505-97d8-4258-a318-209e2493f7eb" => Some("Selection"),
        "2988b2a5-c887-4928-94ae-5d0c9c31e999" => Some("DetailProcessing"),
        "390d5e4b-e732-4c88-8748-9e211a416984" => Some("OnReadAtServer"),
        "3c3da18f-fc18-4f77-8c2d-96c25bec40a5" => Some("Selection"),
        "40925042-2517-455b-a600-d68282829334" => Some("BeforeLoadUserSettingsAtServer"),
        "499bb7af-6262-4de4-819f-ef264d1a20ec" => Some("OnSaveVariantAtServer"),
        "4d88756d-bad4-4fde-92e1-c1f1402ac6b2" => Some("BeforeEditEnd"),
        "53325f0c-b112-4c44-ab12-5d1ee0b1f07b" => Some("DocumentComplete"),
        "674956b3-e469-4fdc-acf5-24ebf88cf7ab" => Some("URLGetProcessing"),
        "6e973761-8683-47fa-a609-4e230950294d" => Some("OnActivateField"),
        "7b15b3db-1cd0-4e1d-a74b-2c972c9e2226" => Some("OnLoadUserSettingsAtServer"),
        "7c39b7bc-db0f-4410-9d98-8e5b7896995e" => Some("BeforeExpand"),
        "87ce636e-9de6-4e42-9395-f0f189d08397" => Some("OnLoadVariantAtServer"),
        "8a5894c9-d2ff-4c1d-b433-89cc352bbfbc" => Some("BeforeWrite"),
        "8bfdb5eb-62dc-4851-8a2c-e983526356bf" => Some("ChoiceProcessing"),
        "8f42e083-be92-4102-b1f0-fa58452c1a63" => Some("BeforeWriteAtServer"),
        "93dfba16-26db-46f8-acb5-4f92f50c855f" => Some("NavigationProcessing"),
        "961ee7c6-0327-422b-adcb-97a90c46753d" => Some("OnSaveUserSettingsAtServer"),
        "9cc34712-da5f-4faa-a653-343d2085fbe8" => Some("BeforeWrite"),
        "a7a9dc42-29b6-4c5b-8980-6d0b87149bdd" => Some("BeforeCollapse"),
        "aeba313d-c467-44b3-b4a2-956340932c8f" => Some("Creating"),
        "b3b65989-73ac-4db3-b6cb-398cb41a062f" => Some("StartListChoice"),
        "b7646583-04d3-4905-8f04-8985914bd1b7" => Some("BeforeWrite"),
        "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6" => Some("BeforeWriteAtServer"),
        "c1bc0d3e-d35e-4207-a06b-ece68ed25314" => Some("OnWriteAtServer"),
        "c331eb1b-d32b-4533-844c-1276600b64e3" => Some("TextEditEnd"),
        "ce67decf-16b8-4d61-b347-4e6a063580dc" => Some("NewWriteProcessing"),
        "d6b86f20-722b-4fe6-83fa-85c6aa4c1fe5" => Some("OnMainServerAvailabilityChange"),
        "d817bccf-504e-4133-a79a-dd16e3a4df73" => Some("OnUpdateUserSettingSetAtServer"),
        "da8dfb86-c5d1-4e35-a8a4-01b167a60ad3" => Some("Click"),
        "e0cd9bdf-88fa-428c-9f1f-86f7f73b11e2" => Some("URLProcessing"),
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
        "fe115cc8-9e33-4684-a166-bd5136fe7a9f" => Some("OnChange"),
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
            && let Some((item_name, property_name)) = form_item_picture_owner_at(text, marker_start)
        {
            assets.push(FormItemAsset {
                item_name,
                file_name: form_item_picture_file_name(property_name, &content),
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

pub(super) fn form_item_picture_owner_at(
    text: &str,
    marker_start: usize,
) -> Option<(String, &'static str)> {
    for (start, _) in text[..marker_start].match_indices('{').rev() {
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
        if form_child_item_id(&fields).is_none() {
            continue;
        }
        let Some(property_name) =
            form_item_picture_property_at(text, marker_start, wrapper, &fields)
        else {
            continue;
        };
        if let Some(item_name) = parse_form_child_item_name(wrapper, &fields) {
            return Some((item_name, property_name));
        }
    }
    None
}

pub(super) fn form_item_picture_property_at(
    text: &str,
    marker_start: usize,
    wrapper: &str,
    fields: &[&str],
) -> Option<&'static str> {
    let tag = form_child_item_tag(wrapper, fields)?;
    match (wrapper, tag) {
        ("12", "PictureDecoration") => {
            let options = fields
                .get(18)
                .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
            if options.first().map(|field| field.trim()) != Some("4") {
                return None;
            }
            form_item_picture_value_matches_marker(text, marker_start, options.get(1)?)
                .then_some("Picture")
        }
        ("31" | "34", "Button") if form_button_layout_is_extended(fields) => {
            let index = 25 + form_button_top_level_offset(fields);
            form_item_picture_value_matches_marker(text, marker_start, fields.get(index)?)
                .then_some("Picture")
        }
        ("55", "Table") => {
            form_item_picture_value_matches_marker(text, marker_start, fields.get(44)?)
                .then_some("RowsPicture")
        }
        ("37", "PictureField") => {
            let input_offset = form_input_field_top_level_offset(fields);
            if form_item_picture_value_matches_marker(
                text,
                marker_start,
                fields.get(29 + input_offset)?,
            ) {
                return Some("HeaderPicture");
            }
            let options = fields
                .get(39 + input_offset)
                .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
            if options.first().map(|field| field.trim()) != Some("10") {
                return None;
            }
            form_item_picture_value_matches_marker(text, marker_start, options.get(5)?)
                .then_some("ValuesPicture")
        }
        _ => None,
    }
}

pub(super) fn form_item_picture_value_matches_marker(
    text: &str,
    marker_start: usize,
    value: &str,
) -> bool {
    let Some(fields) = split_1c_braced_fields(value.trim(), 0) else {
        return false;
    };
    if fields.first().map(|field| field.trim()) != Some("4")
        || fields.get(1).map(|field| field.trim()) != Some("3")
    {
        return false;
    }
    let Some(payload_fields) = fields
        .get(7)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    else {
        return false;
    };
    let [marker] = payload_fields.as_slice() else {
        return false;
    };
    let marker = marker.trim();
    let Some(payload) = marker
        .strip_prefix("{#base64:")
        .and_then(|marker| marker.strip_suffix('}'))
    else {
        return false;
    };
    let Some(content) = decode_base64_mime(payload) else {
        return false;
    };
    if !is_form_item_picture_content(&content) {
        return false;
    }
    let Some(actual_start) = (marker.as_ptr() as usize).checked_sub(text.as_ptr() as usize) else {
        return false;
    };
    actual_start == marker_start
}

pub(super) fn form_item_picture_file_name(property_name: &str, content: &[u8]) -> String {
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
) -> BTreeMap<String, Vec<FormAttributeSaveFieldBinding>> {
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
    let exact_single_type_uuid = fields
        .get(5)
        .and_then(|field| parse_form_exact_single_type_uuid(field));
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
    let settings = settings.map(normalize_form_dynamic_list_settings);
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
        exact_single_type_uuid,
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

fn parse_form_exact_single_type_uuid(field: &str) -> Option<String> {
    let pattern = split_1c_braced_fields(field.trim(), 0)?;
    if pattern.len() != 2 || pattern.first()?.trim() != r#""Pattern""# {
        return None;
    }
    let value_type = split_1c_braced_fields(pattern.get(1)?.trim(), 0)?;
    if value_type.len() != 2 || value_type.first()?.trim() != r##""#""## {
        return None;
    }
    parse_non_zero_uuid(value_type.get(1)?.trim())
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
    mut settings: FormDynamicListSettings,
) -> FormDynamicListSettings {
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
    let Some(entries) = field.and_then(parse_form_attribute_save_entries) else {
        return Vec::new();
    };
    entries
        .iter()
        .any(|entry| matches!(entry, FormAttributeSaveEntry::SelfValue))
        .then(|| vec![attribute_name.to_string()])
        .unwrap_or_default()
}

fn parse_form_attribute_save_entries(field: &str) -> Option<Vec<FormAttributeSaveEntry>> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != 2 + count {
        return None;
    }
    fields
        .iter()
        .skip(2)
        .map(|field| {
            let entry = split_1c_braced_fields(field.trim(), 0)?;
            match entry.as_slice() {
                [kind] if kind.trim() == "0" => Some(FormAttributeSaveEntry::SelfValue),
                [kind, payload] if kind.trim() == "1" => {
                    let key = parse_form_binding_key(payload.trim())?;
                    let payload_fields = split_1c_braced_fields(payload.trim(), 0)?;
                    let metadata_uuid = match payload_fields.as_slice() {
                        [kind, uuid] if kind.trim() == "0" => parse_non_zero_uuid(uuid.trim()),
                        _ => None,
                    };
                    Some(FormAttributeSaveEntry::Binding(
                        FormAttributeSaveFieldBinding { key, metadata_uuid },
                    ))
                }
                _ => None,
            }
        })
        .collect()
}

pub(super) fn parse_form_attribute_save_field_bindings(
    field: Option<&str>,
) -> Vec<FormAttributeSaveFieldBinding> {
    let Some(fields) = field.and_then(|value| split_1c_braced_fields(value.trim(), 0)) else {
        return Vec::new();
    };
    let Some(entries) = fields
        .get(9)
        .and_then(|value| parse_form_attribute_save_entries(value))
    else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter_map(|entry| match entry {
            FormAttributeSaveEntry::SelfValue => None,
            FormAttributeSaveEntry::Binding(binding) => Some(binding),
        })
        .collect()
}

pub(super) fn apply_form_attribute_save_field_bindings(
    attributes: &mut [FormAttribute],
    save_field_bindings: &BTreeMap<String, Vec<FormAttributeSaveFieldBinding>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) {
    for attribute in attributes {
        let metadata_owner = form_attribute_metadata_owner(attribute);
        let mut seen = attribute
            .save_fields
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if let Some(bindings) = save_field_bindings.get(&attribute.name) {
            for binding in bindings {
                let data_path = data_path_by_binding_key
                    .get(&binding.key)
                    .cloned()
                    .or_else(|| {
                        resolve_form_attribute_save_metadata_path(
                            &metadata_owner,
                            binding.metadata_uuid.as_deref()?,
                            object_refs,
                        )
                    });
                if let Some(data_path) = data_path
                    && seen.insert(data_path.clone())
                {
                    attribute.save_fields.push(data_path);
                }
            }
        }
        attribute.save_fields.sort();
        attribute.save_fields.dedup();
    }
}

fn resolve_form_attribute_save_metadata_path(
    attribute: &FormAttributeMetadataOwner,
    metadata_uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let reference = object_refs.get(metadata_uuid)?;
    let (owner_base, relative_path) = form_metadata_data_path_route(reference)?;
    form_attribute_matches_metadata_owner(attribute, &owner_base)
        .then(|| format!("{}.{}", attribute.name, relative_path))
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
    _child_item_indexes: &FormChildItemIndexes,
) -> Option<ParsedFormAttributeAdditionalColumnsGroup> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let target = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let owner = split_1c_braced_fields(target.get(1)?.trim(), 0)?;
    let binding = split_1c_braced_fields(target.get(2)?.trim(), 0)?;
    let schema = FormAttributeAdditionalColumnsGroupSchema::from_raw_layout(
        &fields, &target, &owner, &binding,
    )?;
    let attribute_id = owner.first()?.trim().to_string();
    let attribute = attributes
        .iter()
        .find(|attribute| attribute.id == attribute_id)?;
    let columns = fields
        .iter()
        .skip(3)
        .take(schema.column_count())
        .map(|field| parse_form_attribute_column(field, type_index, object_refs))
        .collect::<Option<Vec<_>>>()?;
    let table = match schema.binding_kind() {
        FormAttributeAdditionalColumnsBindingKind::Numeric => {
            let column_id = binding.first()?.trim();
            attribute
                .columns
                .iter()
                .find(|column| column.id == column_id)
                .map(|column| format!("{}.{}", attribute.name, column.name))
        }
        FormAttributeAdditionalColumnsBindingKind::MetadataReference => {
            resolve_form_attribute_additional_columns_metadata_table_path(
                &attribute_id,
                binding.get(1)?.trim(),
                attributes,
                object_refs,
            )
        }
    }?;
    Some(ParsedFormAttributeAdditionalColumnsGroup {
        attribute_id,
        table,
        columns,
    })
}

fn resolve_form_attribute_additional_columns_metadata_table_path(
    attribute_id: &str,
    type_id: &str,
    attributes: &[FormAttribute],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let type_id = parse_non_zero_uuid(type_id)?;
    let reference = object_refs.get(&type_id)?;
    let (owner_base, relative_path) = form_attribute_additional_columns_metadata_route(reference)?;
    let attribute_owners = form_attribute_metadata_owners_by_id(attributes);
    let attribute = attribute_owners.get(attribute_id)?;
    form_attribute_matches_metadata_owner(attribute, &owner_base)
        .then(|| format!("{}.{}", attribute.name, relative_path))
}

fn form_attribute_additional_columns_metadata_route(reference: &str) -> Option<(String, String)> {
    let route = reference.split('.').collect::<Vec<_>>();
    let (owner_kind, owner_name, relative_path) = match route.as_slice() {
        [owner_kind, owner_name, "TabularSection", table] if !table.is_empty() => {
            (*owner_kind, *owner_name, (*table).to_string())
        }
        [owner_kind, owner_name, "Attribute", attribute] if !attribute.is_empty() => {
            (*owner_kind, *owner_name, (*attribute).to_string())
        }
        _ => return None,
    };
    if owner_kind.is_empty() || owner_name.is_empty() {
        return None;
    }
    Some((format!("{owner_kind}.{owner_name}"), relative_path))
}

pub(super) fn parse_form_attribute_column(
    field: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAttributeColumn> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    FormAttributeColumnSchema::from_raw_layout(&fields)?;
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
        .or_else(|| parse_form_attribute_column_builtin_type_pattern(field))
}

fn parse_form_attribute_column_builtin_type_pattern(field: &str) -> Option<Vec<ConstantValueType>> {
    let pattern = split_1c_braced_fields(field.trim(), 0)?;
    if pattern.len() != 2 || pattern.first()?.trim() != r#""Pattern""# {
        return None;
    }
    let value_type = split_1c_braced_fields(pattern.get(1)?.trim(), 0)?;
    if value_type.len() != 2 || value_type.first()?.trim() != r##""#""## {
        return None;
    }
    let type_id = parse_non_zero_uuid(value_type.get(1)?.trim())?;
    let reference = form_attribute_column_builtin_type_reference(&type_id)?;
    Some(vec![ConstantValueType::Reference {
        reference: reference.to_string(),
    }])
}

pub(super) fn parse_form_dynamic_list_settings(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormDynamicListSettings> {
    let settings_fields = split_1c_braced_fields(field.trim(), 0)?;
    let mut auto_save_user_settings = false;
    let mut manual_query = false;
    let mut manual_query_explicit = false;
    let mut dynamic_data_read = false;
    let mut dynamic_data_read_explicit = false;
    let mut query_text = None;
    let mut main_table = None;
    let mut main_table_category = None;
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
            "MainTableCategory" => main_table_category = parse_form_setting_number(window[1]),
            "Field" => {
                let parsed_fields = parse_form_dynamic_list_fields(window[1]);
                explicit_fields.extend(parsed_fields.clone());
                fields.extend(parsed_fields);
            }
            "AutoSaveUserSettings" => {
                auto_save_user_settings = parse_form_setting_bool(window[1]).unwrap_or(false)
            }
            "ManualQuery" => {
                manual_query_explicit = true;
                manual_query = parse_form_setting_bool(window[1]).unwrap_or(false);
            }
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
    main_table = normalize_form_main_table_category(main_table, main_table_category.as_deref());
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
        manual_query_explicit,
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
        "-8" => Some(format!("{attribute_name}.RegisterRecords")),
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
    let inner = normalize_form_list_settings_raw_fragment(&inner);
    Some(normalize_form_list_settings_raw_fragment(&format!(
        "<dcsset:{emitted_name}>{inner}</dcsset:{emitted_name}>"
    )))
}

pub(super) fn normalize_form_list_settings_raw_fragment(fragment: &str) -> String {
    split_adjacent_xml_tags(
        &fragment
            .replace(
                r#" xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core""#,
                "",
            )
            .replace(
                r#" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings""#,
                "",
            )
            .replace(r#" xmlns:xs="http://www.w3.org/2001/XMLSchema""#, "")
            .replace(
                r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#,
                "",
            ),
    )
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
    .replace(
        r#"<NumberQualifiers xmlns="http://v8.1c.ru/8.1/data/core">"#,
        "<v8:NumberQualifiers>",
    )
    .replace("</NumberQualifiers>", "</v8:NumberQualifiers>")
    .replace("<Digits>", "<v8:Digits>")
    .replace("</Digits>", "</v8:Digits>")
    .replace("<FractionDigits>", "<v8:FractionDigits>")
    .replace("</FractionDigits>", "</v8:FractionDigits>")
    .replace("<AllowedSign>", "<v8:AllowedSign>")
    .replace("</AllowedSign>", "</v8:AllowedSign>")
    .replace(
        r#"<StringQualifiers xmlns="http://v8.1c.ru/8.1/data/core">"#,
        "<v8:StringQualifiers>",
    )
    .replace("</StringQualifiers>", "</v8:StringQualifiers>")
    .replace("<Length>", "<v8:Length>")
    .replace("</Length>", "</v8:Length>")
    .replace("<AllowedLength>", "<v8:AllowedLength>")
    .replace("</AllowedLength>", "</v8:AllowedLength>")
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

pub(super) fn normalize_form_main_table_category(
    main_table: Option<String>,
    category: Option<&str>,
) -> Option<String> {
    let main_table = main_table?;
    match (main_table.as_str(), category) {
        (value, Some("3"))
            if value.starts_with("AccountingRegister.")
                && !value.ends_with(".RecordsWithExtDimensions") =>
        {
            Some(format!("{value}.RecordsWithExtDimensions"))
        }
        _ => Some(main_table),
    }
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
    item_name_by_id: &BTreeMap<String, String>,
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
        .filter_map(|field| parse_form_command_with_items(field, object_refs, item_name_by_id))
        .collect()
}

#[cfg(test)]
pub(super) fn parse_form_command(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommand> {
    parse_form_command_with_items(field, object_refs, &BTreeMap::new())
}

fn parse_form_command_with_items(
    field: &str,
    object_refs: &BTreeMap<String, String>,
    item_name_by_id: &BTreeMap<String, String>,
) -> Option<FormCommand> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let picture_value = split_1c_braced_fields(fields.get(7)?.trim(), 0)?;
    let picture_reference = split_1c_braced_fields(picture_value.get(2)?.trim(), 0)?;
    let schema = FormCommandSchema::from_raw_layout(&fields, &picture_value, &picture_reference)?;
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
    let picture = schema.picture();
    let picture_ref = match picture.kind() {
        FormPictureValueKind::Empty => None,
        FormPictureValueKind::Reference => {
            parse_common_command_picture_value(fields.get(7)?, object_refs)?.0
        }
        _ => return None,
    };
    let current_row_use = schema.current_row_use();
    let associated_table_element_id = schema
        .associated_table_element_id()
        .and_then(|id| item_name_by_id.get(id))
        .cloned();
    let current_row_use = (current_row_use.is_some() || associated_table_element_id.is_some())
        .then_some(FormCommandCurrentRowProperties {
            value: current_row_use,
            associated_table_element_id,
        });
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
        picture_ref,
        picture_load_transparent: picture.load_transparent(),
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
        current_row_use,
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

pub(super) fn parse_form_type_pattern(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    parse_form_metadata_type_pattern(field, object_refs).map(normalize_form_type_pattern)
}

pub(super) fn normalize_form_type_pattern(
    value_types: Vec<ConstantValueType>,
) -> Vec<ConstantValueType> {
    let normalized = value_types
        .into_iter()
        .map(|value_type| match value_type {
            ConstantValueType::Reference { reference }
                if metadata_reference_is_type_set(&reference) =>
            {
                ConstantValueType::ReferenceTypeSet { reference }
            }
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
        .collect::<Vec<_>>();
    let (mut types, type_sets): (Vec<_>, Vec<_>) = normalized
        .into_iter()
        .partition(|value_type| form_metadata_type_xml_tag(value_type) != "TypeSet");
    types.extend(type_sets);
    types
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
    let attribute_metadata_owners_by_id = form_attribute_metadata_owners_by_id(attributes);
    let mut items = parse_form_child_item_pairs(
        fields,
        main_data_path,
        None,
        None,
        &attribute_names_by_id,
        &attribute_metadata_owners_by_id,
        &indexes.table_name_by_id,
        &indexes.standard_command_owner_name_by_id,
        &indexes.command_source_owner_name_by_id,
        &indexes.table_column_names_by_id,
        &indexes.type_link_data_path_by_table_column,
        &indexes.data_path_by_binding_key,
        &indexes.bound_table_path_by_binding_key,
        &indexes.table_column_names_by_binding_key,
        &indexes.owner_scoped_bindings,
        commands,
        object_refs,
    )
    .unwrap_or_default();
    apply_form_table_user_settings_groups(&mut items, &indexes.user_settings_group_by_table_id);
    items
}

fn form_attribute_metadata_owners_by_id(
    attributes: &[FormAttribute],
) -> BTreeMap<String, FormAttributeMetadataOwner> {
    attributes
        .iter()
        .map(|attribute| {
            (
                attribute.id.clone(),
                form_attribute_metadata_owner(attribute),
            )
        })
        .collect()
}

fn form_attribute_metadata_owner(attribute: &FormAttribute) -> FormAttributeMetadataOwner {
    let exact_single_type_reference = match attribute.value_types.as_slice() {
        [ConstantValueType::Reference { reference }] => Some(reference.clone()),
        _ => None,
    };
    let type_references = attribute
        .value_types
        .iter()
        .filter_map(|value_type| match value_type {
            ConstantValueType::Reference { reference } => Some(reference.clone()),
            _ => None,
        })
        .collect();
    let main_table = attribute
        .settings
        .as_ref()
        .and_then(|settings| settings.main_table.clone());
    FormAttributeMetadataOwner {
        name: attribute.name.clone(),
        type_references,
        exact_single_type_reference,
        has_dynamic_list_settings: attribute.settings.is_some(),
        main_table,
    }
}

struct FormAttributePlatformColumn {
    id: &'static str,
    name: &'static str,
}

// Platform-owned columns of built-in value types, independent of infobase metadata.
const FORM_DYNAMIC_LIST_TYPE_UUID: &str = "65abad24-838b-4987-8b35-ed9e2bd4d9c8";
const FORM_VALUE_LIST_TYPE_UUID: &str = "4772b3b4-f4a3-49c0-a1a5-8cb5961511a3";
const FORM_VALUE_LIST_PRESENTATION_COLUMN: FormAttributePlatformColumn =
    FormAttributePlatformColumn {
        id: "1",
        name: "Presentation",
    };
const FORM_VALUE_LIST_CHECK_COLUMN: FormAttributePlatformColumn = FormAttributePlatformColumn {
    id: "2",
    name: "Check",
};
const FORM_VALUE_LIST_PICTURE_COLUMN: FormAttributePlatformColumn = FormAttributePlatformColumn {
    id: "3",
    name: "Picture",
};
const FORM_DYNAMIC_LIST_DEFAULT_PICTURE_COLUMN: FormAttributePlatformColumn =
    FormAttributePlatformColumn {
        id: "10000000",
        name: "DefaultPicture",
    };
const FORM_SETTINGS_COMPOSER_USER_SETTINGS_COLUMN: FormAttributePlatformColumn =
    FormAttributePlatformColumn {
        id: "1",
        name: "UserSettings",
    };
const FORM_SETTINGS_COMPOSER_FIELD_PICTURE_COLUMN: FormAttributePlatformColumn =
    FormAttributePlatformColumn {
        id: "10001",
        name: "FieldPicture",
    };

const FORM_VALUE_LIST_DATA_PATH_COLUMNS: &[FormAttributePlatformColumn] = &[
    FORM_VALUE_LIST_PRESENTATION_COLUMN,
    FORM_VALUE_LIST_CHECK_COLUMN,
];
const FORM_DYNAMIC_LIST_DATA_PATH_COLUMNS: &[FormAttributePlatformColumn] =
    &[FORM_DYNAMIC_LIST_DEFAULT_PICTURE_COLUMN];
const FORM_SETTINGS_COMPOSER_DATA_PATH_COLUMNS: &[FormAttributePlatformColumn] =
    &[FORM_SETTINGS_COMPOSER_USER_SETTINGS_COLUMN];

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
    let platform_column = if form_attribute_is_value_list(attribute) {
        Some(&FORM_VALUE_LIST_PICTURE_COLUMN)
    } else if form_attribute_is_settings_composer(attribute) {
        Some(&FORM_SETTINGS_COMPOSER_FIELD_PICTURE_COLUMN)
    } else {
        None
    };
    if let Some(column) = platform_column {
        table_column_names_by_id
            .entry(attribute.id.clone())
            .or_default()
            .insert(column.id.to_string(), column.name.to_string());
    }
}

fn form_attribute_is_value_list(attribute: &FormAttribute) -> bool {
    attribute.value_types.iter().any(|value_type| {
        matches!(
            value_type,
            ConstantValueType::Reference { reference }
                if matches!(reference.as_str(), "v8:ValueListType" | "ValueListType")
        )
    })
}

fn form_attribute_is_settings_composer(attribute: &FormAttribute) -> bool {
    attribute.value_types.iter().any(|value_type| {
        matches!(
            value_type,
            ConstantValueType::Reference { reference }
                if reference == DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME
        )
    })
}

fn form_attribute_has_exact_single_reference_type(
    attribute: &FormAttribute,
    expected_uuid: &str,
) -> bool {
    attribute
        .exact_single_type_uuid
        .as_deref()
        .is_some_and(|uuid| uuid.eq_ignore_ascii_case(expected_uuid))
}

fn form_attribute_is_exact_dynamic_list(attribute: &FormAttribute) -> bool {
    form_attribute_has_exact_single_reference_type(attribute, FORM_DYNAMIC_LIST_TYPE_UUID)
}

fn form_attribute_is_exact_value_list(attribute: &FormAttribute) -> bool {
    form_attribute_has_exact_single_reference_type(attribute, FORM_VALUE_LIST_TYPE_UUID)
}

fn form_attribute_is_exact_settings_composer(attribute: &FormAttribute) -> bool {
    form_attribute_has_exact_single_reference_type(
        attribute,
        DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_UUID,
    )
}

fn parse_exact_form_attribute_binding_id(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.len() != 2 || fields.first().map(|field| field.trim()) != Some("1") {
        return None;
    }
    let ids = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    if ids.len() != 1 || ids.first()?.trim().is_empty() {
        return None;
    }
    Some(ids.first()?.trim().to_string())
}

#[derive(Default)]
pub(super) struct FormChildItemIndexes {
    pub(super) table_name_by_id: BTreeMap<String, String>,
    pub(super) standard_command_owner_name_by_id: BTreeMap<String, FormStandardCommandOwner>,
    pub(super) table_column_names_by_id: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) bound_table_path_by_binding_key: BTreeMap<String, String>,
    pub(super) table_column_names_by_binding_key: BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: FormOwnerScopedBindingIndexes,
    pub(super) data_path_by_binding_key: BTreeMap<String, String>,
    pub(super) attribute_name_by_binding_key: BTreeMap<String, String>,
    pub(super) binding_names_by_key: BTreeMap<String, BTreeSet<String>>,
    pub(super) item_name_by_id: BTreeMap<String, String>,
    pub(super) command_source_owner_name_by_id: BTreeMap<String, String>,
    pub(super) user_settings_group_id_by_table_id: BTreeMap<String, String>,
    pub(super) user_settings_group_by_table_id: BTreeMap<String, String>,
    bound_attribute_id_by_table_id: BTreeMap<String, String>,
    pub(super) type_link_data_path_by_table_column: BTreeMap<(String, String), String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FormBoundTableKey {
    attribute_id: String,
    table_key: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FormBoundColumnKey {
    attribute_id: String,
    table_key: String,
    column_key: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FormAttributeColumnKey {
    attribute_id: String,
    column_id: String,
}

#[derive(Default)]
pub(super) struct FormOwnerScopedBindingIndexes {
    attribute_columns: BTreeMap<FormAttributeColumnKey, Option<String>>,
    table_paths: BTreeMap<FormBoundTableKey, Option<String>>,
    column_names: BTreeMap<FormBoundColumnKey, Option<String>>,
}

fn collect_form_attribute_data_path_columns(
    index: &mut BTreeMap<FormAttributeColumnKey, Option<String>>,
    attribute: &FormAttribute,
) {
    let mut insert = |column_id: &str, column_name: &str| {
        insert_unambiguous_form_binding(
            index,
            FormAttributeColumnKey {
                attribute_id: attribute.id.clone(),
                column_id: column_id.to_string(),
            },
            column_name.to_string(),
        );
    };
    for column in &attribute.columns {
        insert(&column.id, &column.name);
    }
    let is_dynamic_list = form_attribute_is_exact_dynamic_list(attribute);
    if is_dynamic_list {
        if let Some(settings) = &attribute.settings {
            for field in &settings.fields {
                if let Some(item_id) = &field.item_id {
                    insert(item_id, &field.field);
                }
            }
        }
    }
    let platform_columns = if is_dynamic_list {
        FORM_DYNAMIC_LIST_DATA_PATH_COLUMNS
    } else if form_attribute_is_exact_value_list(attribute) {
        FORM_VALUE_LIST_DATA_PATH_COLUMNS
    } else if form_attribute_is_exact_settings_composer(attribute) {
        FORM_SETTINGS_COMPOSER_DATA_PATH_COLUMNS
    } else {
        &[]
    };
    for column in platform_columns {
        insert(column.id, column.name);
    }
}

fn insert_unambiguous_form_binding<K: Ord>(
    index: &mut BTreeMap<K, Option<String>>,
    key: K,
    value: String,
) {
    match index.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(Some(value));
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            if entry.get().as_ref() != Some(&value) {
                entry.insert(None);
            }
        }
    }
}

pub(super) struct FormStandardCommandOwner {
    name: String,
    kind: FormStandardCommandOwnerKind,
}

#[derive(Clone, Copy)]
pub(super) enum FormStandardCommandOwnerKind {
    Table,
    SpreadsheetDocument,
    GraphicalSchema,
    FormattedDocument,
}

#[cfg(test)]
pub(super) fn collect_form_child_item_indexes(
    fields: &[&str],
    attributes: &[FormAttribute],
) -> FormChildItemIndexes {
    collect_form_child_item_indexes_with_object_refs(fields, attributes, &BTreeMap::new())
}

fn collect_form_child_item_indexes_with_object_refs(
    fields: &[&str],
    attributes: &[FormAttribute],
    object_refs: &BTreeMap<String, String>,
) -> FormChildItemIndexes {
    let mut indexes = FormChildItemIndexes::default();
    let attribute_names_by_id = attributes
        .iter()
        .map(|attribute| (attribute.id.clone(), attribute.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let attribute_metadata_owners_by_id = form_attribute_metadata_owners_by_id(attributes);
    for field in fields {
        collect_form_child_item_indexes_from_field(
            field,
            &attribute_names_by_id,
            &attribute_metadata_owners_by_id,
            object_refs,
            &mut indexes,
        );
    }
    let unresolved_binding_paths = indexes
        .binding_names_by_key
        .iter()
        .filter(|(binding_key, _)| !indexes.data_path_by_binding_key.contains_key(*binding_key))
        .filter_map(|(binding_key, names)| {
            let attribute_name = indexes.attribute_name_by_binding_key.get(binding_key)?;
            let property_name = normalize_form_data_path_child_name(
                attribute_name,
                &infer_form_bound_property_name(names)?,
            );
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
        collect_form_attribute_data_path_columns(
            &mut indexes.owner_scoped_bindings.attribute_columns,
            attribute,
        );
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
    let type_link_routes = indexes
        .bound_attribute_id_by_table_id
        .iter()
        .filter_map(|(table_id, attribute_id)| {
            let table_name = indexes.table_name_by_id.get(table_id)?;
            let attribute = attributes
                .iter()
                .find(|attribute| attribute.id == *attribute_id)?;
            let mut columns = attribute
                .columns
                .iter()
                .map(|column| (column.id.clone(), column.name.clone()))
                .collect::<BTreeMap<_, _>>();
            if let Some(settings) = &attribute.settings {
                columns.extend(settings.fields.iter().filter_map(|field| {
                    field
                        .item_id
                        .as_ref()
                        .map(|item_id| (item_id.clone(), field.field.clone()))
                }));
            }
            Some((
                table_id.clone(),
                table_name.clone(),
                columns,
                form_attribute_is_value_list(attribute),
            ))
        })
        .collect::<Vec<_>>();
    for (table_id, table_name, columns, value_list) in type_link_routes {
        for (column_id, column_name) in columns {
            let field_name = normalize_form_table_column_name(&table_name, &column_name);
            indexes.type_link_data_path_by_table_column.insert(
                (table_id.clone(), column_id),
                format!("Items.{table_name}.CurrentData.{field_name}"),
            );
        }
        if value_list {
            indexes.type_link_data_path_by_table_column.insert(
                (table_id, "0".to_string()),
                format!("Items.{table_name}.CurrentData.Value"),
            );
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
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    object_refs: &BTreeMap<String, String>,
    indexes: &mut FormChildItemIndexes,
) {
    let Some(raw_fields) = split_1c_braced_fields(field.trim(), 0) else {
        return;
    };
    let wrapper = raw_fields.first().map(|field| field.trim());
    let conditional_table_schema =
        wrapper.and_then(|wrapper| form_conditional_table_schema(wrapper, &raw_fields));
    let normalized_fields = conditional_table_schema.map(|schema| {
        raw_fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| (index != schema.prefix_slot()).then_some(*field))
            .collect::<Vec<_>>()
    });
    let fields = normalized_fields.as_deref().unwrap_or(&raw_fields);
    if let Some(wrapper) = wrapper
        && form_spreadsheet_document_field_layout(wrapper, &fields)
        && let Some(id) = form_child_item_id(&fields)
        && let Some(name) = parse_form_child_item_name(wrapper, &fields)
    {
        indexes.standard_command_owner_name_by_id.insert(
            id.to_string(),
            FormStandardCommandOwner {
                name,
                kind: FormStandardCommandOwnerKind::SpreadsheetDocument,
            },
        );
    }
    if let Some(wrapper) = wrapper
        && form_graphical_schema_field_layout(wrapper, &fields)
        && let Some(id) = form_child_item_id(&fields)
        && let Some(name) = parse_form_child_item_name(wrapper, &fields)
    {
        indexes.standard_command_owner_name_by_id.insert(
            id.to_string(),
            FormStandardCommandOwner {
                name,
                kind: FormStandardCommandOwnerKind::GraphicalSchema,
            },
        );
    }
    if let Some(wrapper) = wrapper
        && form_formatted_document_field_layout(wrapper, &fields)
        && let Some(id) = form_child_item_id(&fields)
        && let Some(name) = parse_form_child_item_name(wrapper, &fields)
    {
        indexes.standard_command_owner_name_by_id.insert(
            id.to_string(),
            FormStandardCommandOwner {
                name,
                kind: FormStandardCommandOwnerKind::FormattedDocument,
            },
        );
    }
    if let Some(wrapper) = wrapper
        && (form_child_item_tag(wrapper, &fields).is_some() || matches!(wrapper, "37" | "48"))
        && let Some(id) = form_child_item_id(&fields)
        && let Some(name) = parse_form_child_item_name(wrapper, &fields)
    {
        indexes
            .command_source_owner_name_by_id
            .insert(id.to_string(), name);
    }
    if let Some(wrapper) = wrapper
        && form_child_item_tag(wrapper, &fields).is_some()
        && let Some(id) = form_child_item_id(&fields)
        && let Some(name) = parse_form_child_item_name(wrapper, &fields)
    {
        let tag = form_child_item_tag(wrapper, &fields).unwrap_or_default();
        indexes.item_name_by_id.insert(id.to_string(), name.clone());
        if tag == "Table" {
            indexes.standard_command_owner_name_by_id.insert(
                id.to_string(),
                FormStandardCommandOwner {
                    name: name.clone(),
                    kind: FormStandardCommandOwnerKind::Table,
                },
            );
            indexes
                .table_name_by_id
                .insert(id.to_string(), name.clone());
            if let Some(attribute_id) = fields
                .get(11)
                .and_then(|field| parse_exact_form_attribute_binding_id(field))
            {
                indexes
                    .bound_attribute_id_by_table_id
                    .insert(id.to_string(), attribute_id);
            }
            if let Some((attribute_id, table_key)) = fields
                .get(11)
                .and_then(|field| parse_form_table_binding(field))
                && let Some(attribute_name) = attribute_names_by_id.get(&attribute_id)
            {
                let table_path = format!("{attribute_name}.{}", indexes.table_name_by_id[id]);
                insert_unambiguous_form_binding(
                    &mut indexes.owner_scoped_bindings.table_paths,
                    FormBoundTableKey {
                        attribute_id,
                        table_key: table_key.clone(),
                    },
                    table_path.clone(),
                );
                indexes
                    .bound_table_path_by_binding_key
                    .insert(table_key, table_path);
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
            if let Some(group_id) =
                parse_form_table_property_bag_number(&fields, TableBagKey::UserSettingsGroup)
            {
                indexes
                    .user_settings_group_id_by_table_id
                    .insert(id.to_string(), group_id);
            }
        }
        if matches!(
            tag,
            "InputField"
                | "LabelField"
                | "CheckBoxField"
                | "PictureField"
                | "TextDocumentField"
                | "CalendarField"
                | "GraphicalSchemaField"
                | "SpreadSheetDocumentField"
                | "HTMLDocumentField"
        ) {
            for binding in form_child_item_binding_fields(tag, &fields) {
                if let Some(binding_key) = parse_form_bound_data_binding_key(binding)
                    && let Some(data_path) = parse_form_bound_data_path_with_metadata_owner(
                        binding,
                        &name,
                        attribute_names_by_id,
                        attribute_metadata_owners_by_id,
                        &indexes.table_name_by_id,
                        &indexes.table_column_names_by_id,
                        &indexes.bound_table_path_by_binding_key,
                        &indexes.table_column_names_by_binding_key,
                        object_refs,
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
                if let Some((attribute_id, table_key, column_key)) =
                    parse_form_nested_table_column_binding(binding)
                {
                    let column_name = if column_key == "-2" {
                        "LineNumber".to_string()
                    } else {
                        name.clone()
                    };
                    insert_unambiguous_form_binding(
                        &mut indexes.owner_scoped_bindings.column_names,
                        FormBoundColumnKey {
                            attribute_id,
                            table_key: table_key.clone(),
                            column_key: column_key.clone(),
                        },
                        column_name.clone(),
                    );
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
            collect_form_child_item_indexes_from_field(
                nested,
                attribute_names_by_id,
                attribute_metadata_owners_by_id,
                object_refs,
                indexes,
            );
        }
    }
}

pub(super) fn form_spreadsheet_document_field_layout(wrapper: &str, fields: &[&str]) -> bool {
    matches!(wrapper, "37" | "48")
        && fields
            .get(5 + form_input_field_top_level_offset(fields))
            .is_some_and(|value| value.trim() == "6")
}

pub(super) fn form_graphical_schema_field_layout(wrapper: &str, fields: &[&str]) -> bool {
    wrapper == "37"
        && fields
            .get(5 + form_input_field_top_level_offset(fields))
            .is_some_and(|value| value.trim() == "14")
}

pub(super) fn form_formatted_document_field_layout(wrapper: &str, fields: &[&str]) -> bool {
    matches!(wrapper, "37" | "48")
        && fields
            .get(5 + form_input_field_top_level_offset(fields))
            .is_some_and(|value| value.trim() == "17")
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
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    item_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
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
            if let Some(item) = parse_form_child_item_with_metadata_owners(
                field,
                main_data_path,
                parent_data_path,
                parent_tag,
                attribute_names_by_id,
                attribute_metadata_owners_by_id,
                table_name_by_id,
                standard_command_owner_name_by_id,
                item_name_by_id,
                table_column_names_by_id,
                type_link_data_path_by_table_column,
                data_path_by_binding_key,
                bound_table_path_by_binding_key,
                table_column_names_by_binding_key,
                owner_scoped_bindings,
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
    const ROOT_TRAILER_FIELDS: usize = 24;
    if fields.first().map(|field| field.trim()) != Some("50") {
        return None;
    }
    let tail_start = fields.len().checked_sub(ROOT_TRAILER_FIELDS)?;
    let mut matched_tail = None;
    let max_count = tail_start.saturating_sub(1) / 2;
    for count in 0usize..=max_count {
        let Some(count_index) = tail_start.checked_sub(1 + count * 2) else {
            continue;
        };
        if fields
            .get(count_index)
            .and_then(|field| field.trim().parse::<usize>().ok())
            != Some(count)
        {
            continue;
        }
        let mut complete = true;
        for item_index in 0..count {
            let uuid_index = count_index + 1 + item_index * 2;
            let value_index = uuid_index + 1;
            if fields
                .get(uuid_index)
                .and_then(|field| parse_non_zero_uuid(field.trim()))
                .is_none()
                || fields
                    .get(value_index)
                    .and_then(|field| split_1c_braced_fields(field.trim(), 0))
                    .is_none()
            {
                complete = false;
                break;
            }
        }
        if complete {
            if matched_tail.replace(tail_start).is_some() {
                return None;
            }
        }
    }
    matched_tail
}

pub(super) fn form_root_child_item_pairs_tail_start(fields: &[&str]) -> Option<usize> {
    form_root_child_items_tail_start(fields)
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
    parse_form_child_item_with_metadata_owners(
        field,
        main_data_path,
        parent_data_path,
        None,
        attribute_names_by_id,
        &BTreeMap::new(),
        table_name_by_id,
        &BTreeMap::new(),
        &BTreeMap::new(),
        table_column_names_by_id,
        &BTreeMap::new(),
        &BTreeMap::new(),
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        &FormOwnerScopedBindingIndexes::default(),
        commands,
        object_refs,
    )
}

#[cfg(test)]
pub(super) fn parse_form_child_item_with_context(
    field: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    item_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    parse_form_child_item_with_metadata_owners(
        field,
        main_data_path,
        parent_data_path,
        parent_tag,
        attribute_names_by_id,
        &BTreeMap::new(),
        table_name_by_id,
        standard_command_owner_name_by_id,
        item_name_by_id,
        table_column_names_by_id,
        &BTreeMap::new(),
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        &FormOwnerScopedBindingIndexes::default(),
        commands,
        object_refs,
    )
}

fn parse_form_child_item_with_metadata_owners(
    field: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    _parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    item_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    let raw_fields = split_1c_braced_fields(field.trim(), 0)?;
    let wrapper = raw_fields.first()?.trim();
    let conditional_group_schema = form_conditional_group_schema(wrapper, &raw_fields);
    let conditional_table_schema = form_conditional_table_schema(wrapper, &raw_fields);
    let conditional_user_visible_common = conditional_group_schema
        .map(|_| false)
        .or_else(|| conditional_table_schema.map(|_| false));
    let conditional_prefix_slot = conditional_group_schema
        .map(|schema| schema.prefix_slot())
        .or_else(|| conditional_table_schema.map(|schema| schema.prefix_slot()));
    let normalized_fields = conditional_prefix_slot.map(|prefix_slot| {
        raw_fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| (index != prefix_slot).then_some(*field))
            .collect::<Vec<_>>()
    });
    let fields = normalized_fields.as_deref().unwrap_or(&raw_fields);
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id == "0" {
        return None;
    }
    let tag = form_child_item_tag(wrapper, fields)?;
    let name = parse_form_child_item_name(wrapper, fields)?;
    let button_top_level_offset = (tag == "Button")
        .then(|| form_button_top_level_offset(&fields))
        .unwrap_or(0);
    let input_field_extended_options = (matches!(tag, "InputField" | "TextDocumentField")
        && form_input_field_layout_is_extended(&fields))
    .then(|| form_input_field_extended_options(&fields))
    .flatten();
    let picture_field_options = (tag == "PictureField")
        .then(|| parse_form_picture_field_options(&fields))
        .flatten();
    let radio_button_options = (tag == "RadioButtonField")
        .then(|| parse_form_radio_button_options(&fields))
        .flatten();
    let check_box_field_layout = (tag == "CheckBoxField")
        .then(|| parse_form_check_box_field_layout(wrapper, &fields))
        .flatten();
    let special_field_layout = matches!(tag, "ProgressBarField" | "TrackBarField" | "ChartField")
        .then(|| parse_form_special_field_layout(wrapper, &fields))
        .flatten();
    let input_field_top_level_offset = matches!(
        tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
            | "CalendarField"
            | "GraphicalSchemaField"
            | "SpreadSheetDocumentField"
            | "HTMLDocumentField"
            | "ProgressBarField"
            | "TrackBarField"
            | "ChartField"
    )
    .then(|| {
        form_input_field_layout_is_extended(&fields)
            .then(|| form_input_field_top_level_offset(&fields))
    })
    .flatten()
    .unwrap_or(0);
    let display_importance_top_level_offset = if tag == "Button" {
        button_top_level_offset
    } else {
        input_field_top_level_offset
    };
    let display_importance_schema = FormChildItemDisplayImportanceSchema::from_raw_layout(
        wrapper,
        fields.len(),
        tag,
        display_importance_top_level_offset,
    );
    let direct_discriminator = fields
        .get(5 + input_field_top_level_offset)
        .map(|field| field.trim());
    let command_bar_schema = fields
        .get(FormCommandBarSchema::OPTIONS_SLOT)
        .and_then(|field| {
            let options_text = field.trim();
            (scan_1c_braced_value(options_text, 0) == Some(options_text.len()))
                .then(|| split_1c_braced_fields(options_text, 0))
                .flatten()
        })
        .and_then(|options| {
            let source_text = options.get(2)?.trim();
            if scan_1c_braced_value(source_text, 0) != Some(source_text.len()) {
                return None;
            }
            let source = split_1c_braced_fields(source_text, 0)?;
            FormCommandBarSchema::from_raw_layout(
                wrapper,
                tag,
                direct_discriminator,
                &fields,
                &options,
                &source,
            )
        });
    let title_location_schema = FormFieldTitleLocationSchema::from_raw_layout(
        wrapper,
        fields.len(),
        tag,
        input_field_top_level_offset,
        direct_discriminator,
    );
    let shared_container_content_change_schema =
        FormSharedContainerContentChangeSchema::from_raw_layout(
            wrapper,
            fields.len(),
            tag,
            direct_discriminator,
            fields.get(9).map(|field| field.trim()),
        );
    let field_schema_and_options = fields
        .get(FormFieldSchema::OPTIONS_BASE_SLOT + input_field_top_level_offset)
        .and_then(|field| {
            let options_text = field.trim();
            (scan_1c_braced_value(options_text, 0) == Some(options_text.len()))
                .then(|| split_1c_braced_fields(options_text, 0))
                .flatten()
        })
        .and_then(|options| {
            FormFieldSchema::from_raw_layout(
                wrapper,
                fields.len(),
                tag,
                input_field_top_level_offset,
                direct_discriminator,
                &options,
            )
            .map(|schema| (schema, options))
        });
    let spreadsheet_document_properties = field_schema_and_options
        .as_ref()
        .and_then(|(schema, options)| schema.spreadsheet_document_properties(&fields, options));
    let button_color_schema = FormButtonColorSchema::from_raw_layout(wrapper, fields.len(), tag);
    let button_shape_representation_schema = FormButtonShapeRepresentationSchema::from_raw_layout(
        wrapper,
        fields.len(),
        tag,
        button_top_level_offset,
    );
    let button_common_schema = FormButtonCommonSchema::from_raw_layout(
        wrapper,
        fields.len(),
        tag,
        button_top_level_offset,
    );
    let table_schema = FormTableSchema::from_raw_layout(wrapper, tag, &fields);
    let strict_table_root_properties = table_schema
        .and_then(|schema| parse_form_table_root_properties(schema, &fields))
        .unwrap_or_default();
    let button_data_path_slot = button_common_schema.and_then(|schema| schema.data_path_slot());
    let strict_field_data_path = field_schema_and_options.is_some();
    let owner_scoped_data_path =
        strict_field_data_path || table_schema.is_some() || button_data_path_slot.is_some();
    let data_path_resolution = parse_form_child_item_data_path(
        tag,
        &fields,
        &name,
        id,
        main_data_path,
        parent_data_path,
        strict_field_data_path,
        owner_scoped_data_path,
        button_data_path_slot,
        attribute_names_by_id,
        attribute_metadata_owners_by_id,
        table_name_by_id,
        table_column_names_by_id,
        type_link_data_path_by_table_column,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        owner_scoped_bindings,
        object_refs,
    );
    let data_path_provenance = data_path_resolution
        .as_ref()
        .map(|resolved| resolved.provenance);
    let child_parent_data_path = data_path_resolution
        .as_ref()
        .map(|resolved| resolved.data_path.as_str())
        .or(parent_data_path);
    let user_visible_schema = if tag == "Button" {
        FormChildItemUserVisibleSchema::from_raw_layout(
            wrapper,
            fields.len(),
            tag,
            button_top_level_offset,
            fields.get(3).map(|field| field.trim()),
            fields
                .get(4)
                .and_then(|field| parse_form_conditional_user_visible_common(field)),
        )
    } else if tag == "PictureField" {
        FormChildItemUserVisibleSchema::from_raw_layout(
            wrapper,
            fields.len(),
            tag,
            input_field_top_level_offset,
            fields.get(4).map(|field| field.trim()),
            fields
                .get(5)
                .and_then(|field| parse_form_conditional_user_visible_common(field)),
        )
    } else {
        None
    };
    let show_title_options = matches!(tag, "ColumnGroup" | "Page" | "UsualGroup")
        .then(|| {
            fields
                .get(FormChildItemShowTitleSchema::OPTIONS_SLOT)
                .and_then(|field| split_1c_braced_fields(field.trim(), 0))
        })
        .flatten();
    let show_title_schema = show_title_options.as_deref().and_then(|options| {
        FormChildItemShowTitleSchema::from_raw_layout(
            wrapper,
            fields.len(),
            tag,
            direct_discriminator,
            options,
        )
    });
    let container_read_only_schema = matches!(tag, "ColumnGroup" | "Page")
        .then(|| {
            let options_text = fields
                .get(FormChildItemShowTitleSchema::OPTIONS_SLOT)?
                .trim();
            if scan_1c_braced_value(options_text, 0) != Some(options_text.len()) {
                return None;
            }
            let options = split_1c_braced_fields(options_text, 0)?;
            FormContainerReadOnlySchema::from_raw_layout(
                wrapper,
                fields.len(),
                tag,
                direct_discriminator,
                &options,
            )
        })
        .flatten();
    let nested_auto_command_bar_schema = (tag == "AutoCommandBar")
        .then(|| {
            let marker_text = fields.get(20)?.trim();
            if scan_1c_braced_value(marker_text, 0) != Some(marker_text.len()) {
                return None;
            }
            let marker = split_1c_braced_fields(marker_text, 0)?;
            FormNestedAutoCommandBarSchema::from_raw_layout(
                wrapper,
                fields.len(),
                tag,
                id,
                direct_discriminator,
                &marker,
            )
        })
        .flatten();
    let page_schema = show_title_options.as_deref().and_then(|options| {
        FormPageSchema::from_raw_layout(wrapper, fields.len(), tag, direct_discriminator, options)
    });
    let page_properties = page_schema.and_then(|schema| {
        show_title_options
            .as_deref()
            .map(|options| schema.properties(&fields, options))
    });
    let popup_schema = (tag == "Popup")
        .then(|| {
            fields
                .get(FormPopupSchema::OPTIONS_SLOT)
                .and_then(|field| split_1c_braced_fields(field.trim(), 0))
        })
        .flatten()
        .and_then(|options| {
            FormPopupSchema::from_raw_layout(
                wrapper,
                fields.len(),
                tag,
                direct_discriminator,
                &options,
            )
        });
    let mut child_items = parse_form_child_item_pairs(
        &fields,
        main_data_path,
        child_parent_data_path,
        Some(tag),
        attribute_names_by_id,
        attribute_metadata_owners_by_id,
        table_name_by_id,
        standard_command_owner_name_by_id,
        item_name_by_id,
        table_column_names_by_id,
        type_link_data_path_by_table_column,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        owner_scoped_bindings,
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
            attribute_metadata_owners_by_id,
            table_name_by_id,
            standard_command_owner_name_by_id,
            item_name_by_id,
            table_column_names_by_id,
            type_link_data_path_by_table_column,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            owner_scoped_bindings,
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
            attribute_metadata_owners_by_id,
            table_name_by_id,
            standard_command_owner_name_by_id,
            item_name_by_id,
            table_column_names_by_id,
            type_link_data_path_by_table_column,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            owner_scoped_bindings,
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
            attribute_metadata_owners_by_id,
            table_name_by_id,
            standard_command_owner_name_by_id,
            item_name_by_id,
            table_column_names_by_id,
            type_link_data_path_by_table_column,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            owner_scoped_bindings,
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
            attribute_metadata_owners_by_id,
            table_name_by_id,
            standard_command_owner_name_by_id,
            item_name_by_id,
            table_column_names_by_id,
            type_link_data_path_by_table_column,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            owner_scoped_bindings,
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
        .then(|| parse_form_label_decoration_options(tag, &fields, object_refs))
        .flatten();
    let picture_decoration_properties = FormPictureDecorationSchema::from_raw_layout(
        wrapper,
        fields.len(),
        tag,
        direct_discriminator,
    )
    .map(|schema| schema.properties(&fields));
    let label_field_options = (tag == "LabelField")
        .then(|| parse_form_label_field_options(&fields, object_refs))
        .flatten();
    let document_field_options_kind = match tag {
        "CalendarField" => Some("6"),
        "GraphicalSchemaField" | "HTMLDocumentField" => Some("3"),
        _ => None,
    };
    let document_field_options = document_field_options_kind.and_then(|options_kind| {
        fields
            .get(39)
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
            .filter(|options| options.first().map(|field| field.trim()) == Some(options_kind))
    });
    let ordinary_table_layout = tag == "Table" && form_table_ordinary_layout_variant(&fields);
    let table_schema_skip_on_input = table_schema.and_then(|schema| schema.skip_on_input(&fields));
    let command_name = if tag == "Button" {
        fields.get(8 + button_top_level_offset).and_then(|field| {
            parse_form_button_command_name(
                field,
                commands,
                object_refs,
                standard_command_owner_name_by_id,
            )
        })
    } else {
        None
    };
    let (title, title_formatted) = parse_form_child_item_title(
        tag,
        wrapper,
        &fields,
        field_schema_and_options.as_ref().map(|(schema, _)| *schema),
    );
    let input_hint = (tag == "InputField")
        .then(|| parse_form_input_field_input_hint(input_field_extended_options.as_deref()))
        .unwrap_or_default();
    let input_hint =
        if input_field_top_level_offset > 0 && !input_hint.is_empty() && input_hint == title {
            Vec::new()
        } else {
            input_hint
        };
    let audited_input_field_options = (tag == "InputField"
        && wrapper == "37"
        && fields.len() == 59
        && input_field_top_level_offset == 0)
        .then_some(input_field_extended_options.as_deref())
        .flatten();
    let tooltip = parse_form_child_item_tooltip(
        tag,
        wrapper,
        &fields,
        field_schema_and_options.as_ref().map(|(schema, _)| *schema),
        check_box_field_layout.as_ref().map(|(schema, _)| *schema),
        table_schema,
    );
    let tooltip_representation = parse_form_field_tooltip_representation(wrapper, tag, &fields);
    let header_picture = parse_form_field_header_picture(
        wrapper,
        tag,
        &fields,
        input_field_top_level_offset,
        object_refs,
    );
    let choice_button_picture = field_schema_and_options
        .as_ref()
        .and_then(|(schema, options)| {
            parse_form_input_field_choice_button_picture(*schema, options, object_refs)
        });
    let page_picture = page_schema.and_then(|schema| {
        show_title_options
            .as_deref()
            .and_then(|options| parse_form_page_picture(schema, options, object_refs))
    });
    let rows_picture =
        table_schema.and_then(|schema| parse_form_table_rows_picture(schema, &fields, object_refs));
    let mut item = FormChildItem {
        tag,
        id: id.to_string(),
        name,
        display_importance: display_importance_schema
            .and_then(|schema| schema.display_importance(&fields)),
        auto_command_bar_empty_element: is_raw_empty_nested_auto_command_bar(
            wrapper, tag, id, &fields,
        ),
        autofill: if let Some(schema) = table_schema {
            schema.autofill(&fields)
        } else if tag == "ContextMenu" {
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
            page_properties.and_then(|properties| properties.group())
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
        } else if tag == "Popup" {
            popup_schema.and_then(FormPopupSchema::representation)
        } else {
            extended_group_options
                .as_ref()
                .and_then(|options| options.representation)
        },
        usual_group_enabled: extended_group_options
            .as_ref()
            .and_then(|options| options.enabled),
        enable_content_change: page_properties
            .and_then(|properties| properties.enable_content_change())
            .or_else(|| {
                extended_group_options
                    .as_ref()
                    .and_then(|options| options.enable_content_change)
            })
            .or_else(|| {
                shared_container_content_change_schema
                    .and_then(|schema| schema.enable_content_change())
            }),
        child_items_width: page_properties
            .and_then(|properties| properties.child_items_width())
            .or_else(|| {
                extended_group_options
                    .as_ref()
                    .and_then(|options| options.child_items_width)
            }),
        control_representation: extended_group_options
            .as_ref()
            .and_then(|options| options.control_representation),
        collapsed: extended_group_options
            .as_ref()
            .and_then(|options| options.collapsed),
        usual_group_collapsed_representation_title: extended_group_options
            .as_ref()
            .map(|options| options.collapsed_representation_title.clone())
            .unwrap_or_default(),
        usual_group_children_align: extended_group_options
            .as_ref()
            .and_then(|options| options.children_align),
        usual_group_horizontal_spacing: extended_group_options
            .as_ref()
            .and_then(|options| options.horizontal_spacing),
        usual_group_vertical_spacing: extended_group_options
            .as_ref()
            .and_then(|options| options.vertical_spacing),
        usual_group_horizontal_align: page_properties
            .and_then(|properties| properties.horizontal_align())
            .or_else(|| {
                extended_group_options
                    .as_ref()
                    .and_then(|options| options.horizontal_align)
            }),
        usual_group_vertical_align: page_properties
            .and_then(|properties| properties.vertical_align())
            .or_else(|| {
                extended_group_options
                    .as_ref()
                    .and_then(|options| options.vertical_align)
            }),
        usual_group_group_vertical_align: extended_group_options
            .as_ref()
            .and_then(|options| options.group_vertical_align),
        through_align: extended_group_options
            .as_ref()
            .and_then(|options| options.through_align),
        united: extended_group_options
            .as_ref()
            .and_then(|options| options.united),
        usual_group_show_left_margin: extended_group_options
            .as_ref()
            .and_then(|options| options.show_left_margin),
        table_representation: if tag == "Table" {
            parse_form_table_representation_from_fields(wrapper, &fields)
        } else {
            None
        },
        table_command_bar_location: if tag == "Table" {
            parse_form_table_command_bar_location(wrapper, &fields)
        } else {
            None
        },
        table_search_string_location: table_schema
            .and_then(|schema| schema.search_string_location(&fields)),
        table_view_status_location: table_schema
            .and_then(|schema| schema.view_status_location(&fields)),
        table_search_control_location: table_schema
            .and_then(|schema| schema.search_control_location(&fields)),
        height_in_table_rows: if tag == "Table" {
            fields
                .get(21)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else {
            None
        },
        row_selection_mode: if wrapper == "55" {
            table_schema.and_then(|schema| schema.row_selection_mode(&fields))
        } else if tag == "Table" {
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
        enable_start_drag: table_schema.and_then(|schema| schema.enable_start_drag(&fields)),
        enable_drag: table_schema.and_then(|schema| schema.enable_drag(&fields)),
        file_drag_mode: if tag == "Table" {
            if let Some(schema) = table_schema {
                schema.file_drag_mode(&fields)
            } else {
                parse_form_table_file_drag_mode_from_fields(wrapper, &fields)
            }
        } else if tag == "PictureDecoration" {
            parse_form_picture_decoration_file_drag_mode(&fields)
        } else if tag == "PictureField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| schema.picture_field_file_drag_mode(options))
        } else {
            None
        },
        auto_refresh: if tag == "Table" {
            parse_form_table_property_bag_bool(&fields, TableBagKey::AutoRefresh)
        } else {
            None
        },
        auto_refresh_period: if tag == "Table" {
            parse_form_table_property_bag_number(&fields, TableBagKey::AutoRefreshPeriod)
        } else {
            None
        },
        period: if tag == "Table" {
            parse_form_table_period(&fields)
        } else {
            None
        },
        change_row_set: table_schema.and_then(|schema| schema.change_row_set(&fields)),
        change_row_order: table_schema.and_then(|schema| schema.change_row_order(&fields)),
        command_set_excluded_commands: table_schema
            .map(|schema| parse_form_table_command_set_excluded_commands_for_table(schema, &fields))
            .unwrap_or_else(|| {
                parse_form_field_command_set_excluded_commands(wrapper, tag, &fields)
            }),
        use_alternation_row_color: table_schema
            .and_then(|schema| schema.use_alternation_row_color(&fields)),
        default_item: if tag == "Table" {
            table_schema.and_then(|schema| schema.default_item(&fields))
        } else if let Some(properties) = spreadsheet_document_properties.as_ref() {
            properties.default_item
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(13 + button_top_level_offset)
                .and_then(|field| parse_form_child_item_show_title(field))
        } else if matches!(wrapper, "37" | "48")
            && matches!(
                tag,
                "InputField"
                    | "LabelField"
                    | "CheckBoxField"
                    | "PictureField"
                    | "RadioButtonField"
                    | "TextDocumentField"
                    | "ProgressBarField"
                    | "TrackBarField"
                    | "ChartField"
            )
        {
            fields
                .get(FieldSlot::DefaultItem.index(input_field_top_level_offset))
                .and_then(|field| parse_form_child_item_show_title(field))
                .filter(|value| *value)
        } else {
            None
        },
        initial_tree_view: if tag == "Table" {
            parse_form_table_initial_tree_view(wrapper, &fields)
        } else {
            None
        },
        row_input_mode: table_schema.and_then(|schema| schema.row_input_mode(&fields)),
        table_choice_mode: table_schema.and_then(|schema| schema.choice_mode(&fields)),
        table_selection_mode: table_schema.and_then(|schema| schema.selection_mode(&fields)),
        table_header: table_schema.and_then(|schema| schema.header(&fields)),
        table_horizontal_lines: table_schema.and_then(|schema| schema.horizontal_lines(&fields)),
        table_vertical_lines: table_schema.and_then(|schema| schema.vertical_lines(&fields)),
        show_root: if table_schema.is_some() {
            strict_table_root_properties.show_root
        } else if tag == "Table" && !ordinary_table_layout {
            fields
                .get(36)
                .and_then(|field| parse_form_child_item_show_title(field))
                .filter(|value| *value)
        } else {
            None
        },
        allow_root_choice: if table_schema.is_some() {
            strict_table_root_properties.allow_root_choice
        } else if tag == "Table" && !ordinary_table_layout {
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
        restore_current_row: if table_schema.is_some() {
            strict_table_root_properties.restore_current_row
        } else if tag == "Table" {
            parse_form_table_property_bag_bool(&fields, TableBagKey::RestoreCurrentRow)
        } else {
            None
        },
        row_filter_nil: if tag == "Table" {
            if ordinary_table_layout {
                fields
                    .get(56)
                    .and_then(|field| parse_form_standalone_undefined_marker(field))
            } else {
                parse_form_table_property_bag_undefined(&fields, TableBagKey::RowFilter)
            }
        } else {
            None
        },
        row_picture_data_path: if tag == "Table" {
            parse_form_table_property_bag_string(&fields, TableBagKey::RowPictureDataPath)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    table_schema.and_then(|schema| {
                        parse_form_table_row_picture_data_path(
                            schema,
                            &fields,
                            data_path_resolution
                                .as_ref()
                                .map(|resolved| resolved.data_path.as_str()),
                            table_column_names_by_id,
                            attribute_metadata_owners_by_id,
                            object_refs,
                        )
                    })
                })
        } else {
            None
        },
        rows_picture_ref: rows_picture
            .as_ref()
            .and_then(|picture| picture.reference.clone()),
        rows_picture_file_name: rows_picture
            .as_ref()
            .and_then(|picture| picture.file_name.clone()),
        rows_picture_load_transparent: rows_picture
            .as_ref()
            .is_some_and(|picture| picture.load_transparent),
        top_level_parent_nil: if table_schema.is_some() {
            strict_table_root_properties.top_level_parent_nil
        } else if tag == "Table" && !ordinary_table_layout {
            parse_form_table_property_bag_undefined(&fields, TableBagKey::TopLevelParent)
                .or_else(|| parse_form_table_default_top_level_parent_nil(wrapper, &fields))
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
            parse_form_table_property_bag_bool(&fields, TableBagKey::AllowGettingCurrentRowUrl)
        } else {
            None
        },
        button_representation: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(10 + button_top_level_offset)
                .and_then(|field| parse_form_button_representation(field))
        } else {
            None
        },
        shape_representation: button_shape_representation_schema
            .and_then(|schema| schema.shape_representation(&fields)),
        representation_in_context_menu: if tag == "Button"
            && form_button_layout_is_extended(&fields)
        {
            fields
                .get(43 + button_top_level_offset)
                .and_then(|field| parse_form_button_representation_in_context_menu(field))
        } else {
            None
        },
        group_horizontal_align: if let Some(schema) = command_bar_schema {
            schema.group_horizontal_align(&fields)
        } else if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .and_then(|options| options.group_horizontal_align)
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(41 + button_top_level_offset)
                .and_then(|field| parse_form_button_group_horizontal_align(field))
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.group_horizontal_align)
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.group_horizontal_align())
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
                .get(49 + button_top_level_offset)
                .and_then(|field| parse_form_button_location_in_command_bar(field))
        } else {
            None
        },
        default_button: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(11 + button_top_level_offset)
                .and_then(|field| parse_form_child_item_show_title(field))
        } else {
            None
        },
        scroll_on_compress: parse_form_page_scroll_on_compress(tag, &fields),
        show_title: show_title_schema.and_then(|schema| {
            show_title_options
                .as_deref()
                .and_then(|options| schema.show_title(options))
        }),
        show_in_header: if tag == "ColumnGroup" {
            column_group_options
                .as_ref()
                .and_then(|options| options.show_in_header)
        } else {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, _)| schema.show_in_header(&fields))
                .or_else(|| {
                    (matches!(tag, "InputField" | "LabelField" | "CheckBoxField")
                        && form_input_field_layout_is_extended(&fields))
                    .then(|| parse_form_child_item_show_in_header(&fields))
                    .flatten()
                })
        },
        user_visible_common: conditional_user_visible_common
            .or_else(|| user_visible_schema.map(|_| false))
            .or_else(|| {
                (matches!(
                    tag,
                    "InputField"
                        | "LabelField"
                        | "CheckBoxField"
                        | "RadioButtonField"
                        | "TextDocumentField"
                ) && form_input_field_layout_is_extended(&fields)
                    && input_field_top_level_offset > 0)
                    .then_some(false)
            }),
        visible: FormChildItemVisibleSchema::from_raw_layout(
            wrapper,
            fields.len(),
            tag,
            direct_discriminator,
            input_field_top_level_offset,
        )
        .and_then(|schema| schema.visible(&fields)),
        enabled: field_schema_and_options
            .as_ref()
            .and_then(|(schema, _)| schema.enabled(&fields))
            .or_else(|| button_common_schema.and_then(|schema| schema.enabled(&fields)))
            .or_else(|| command_bar_schema.and_then(|schema| schema.enabled(&fields))),
        read_only: if let Some(schema) = container_read_only_schema {
            schema.read_only(&fields)
        } else if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .and_then(|options| options.read_only)
        } else if tag == "Table" {
            table_schema.and_then(|schema| schema.read_only(&fields))
        } else {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, _)| schema.read_only(&fields))
                .or_else(|| {
                    (matches!(
                        tag,
                        "InputField"
                            | "TextDocumentField"
                            | "LabelField"
                            | "GraphicalSchemaField"
                            | "HTMLDocumentField"
                    ) && form_input_field_layout_is_extended(&fields))
                    .then(|| {
                        fields
                            .get(14 + input_field_top_level_offset)
                            .and_then(|field| parse_form_child_item_show_title(field))
                    })
                    .flatten()
                })
        },
        skip_on_input: if tag == "Table" {
            if table_schema.is_some() {
                table_schema_skip_on_input
            } else {
                fields
                    .get(12)
                    .and_then(|field| parse_form_input_field_skip_on_input(field))
            }
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(29 + button_top_level_offset)
                .and_then(|field| parse_form_input_field_skip_on_input(field))
        } else if let Some((schema, _)) = field_schema_and_options.as_ref() {
            schema.skip_on_input(&fields)
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.skip_on_input)
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.skip_on_input())
        } else {
            None
        },
        strict_table_schema: table_schema.is_some(),
        title_location: title_location_schema
            .and_then(|schema| schema.title_location(&fields))
            .or_else(|| table_schema.and_then(|schema| schema.title_location(&fields))),
        title_height: field_schema_and_options
            .as_ref()
            .and_then(|(schema, _)| schema.title_height(&fields))
            .or_else(|| button_common_schema.and_then(|schema| schema.title_height(&fields))),
        tooltip_representation,
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
        horizontal_align: field_schema_and_options
            .as_ref()
            .and_then(|(schema, _)| schema.horizontal_align(&fields))
            .map(FormChildItemAlignment::Horizontal)
            .or_else(|| {
                nested_auto_command_bar_schema
                    .and_then(FormNestedAutoCommandBarSchema::horizontal_align)
                    .map(FormChildItemAlignment::Horizontal)
            })
            .or_else(|| {
                if matches!(tag, "InputField" | "LabelField")
                    && form_input_field_layout_is_extended(&fields)
                {
                    parse_form_input_field_horizontal_align(&fields)
                        .map(FormChildItemAlignment::Horizontal)
                } else if let Some((schema, _)) = check_box_field_layout.as_ref() {
                    schema
                        .horizontal_align(&fields)
                        .map(FormChildItemAlignment::Horizontal)
                } else if tag == "LabelDecoration" {
                    label_decoration_options
                        .as_ref()
                        .map(|options| FormChildItemAlignment::LabelDecoration(options.alignment))
                } else {
                    None
                }
            }),
        group_vertical_align: if let Some(schema) = command_bar_schema {
            schema.group_vertical_align(&fields)
        } else if let Some(schema) = button_common_schema {
            schema.group_vertical_align(&fields)
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.group_vertical_align())
        } else {
            special_field_layout
                .as_ref()
                .and_then(|(schema, _)| schema.group_vertical_align(&fields))
        },
        label_decoration_visual_tail: label_decoration_options
            .as_ref()
            .map(|options| options.visual_tail.clone()),
        check_box_type: check_box_field_layout
            .as_ref()
            .and_then(|(schema, options)| schema.check_box_type(options)),
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
        footer_horizontal_align: field_schema_and_options
            .as_ref()
            .and_then(|(schema, _)| schema.footer_horizontal_align(&fields)),
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
        text_color: if let Some(schema) = button_color_schema {
            fields
                .get(schema.text_color_slot())
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if tag == "InputField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| options.get(schema.text_color_option_slot()?))
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if let Some(schema) = table_schema {
            fields
                .get(schema.text_color_slot())
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if tag == "LabelField"
            && let Some(value) = field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| options.get(schema.text_color_option_slot()?))
                .and_then(|field| parse_form_control_color(field, object_refs))
        {
            Some(value)
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.text_color.clone())
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.text_color.clone())
        } else if tag == "PictureDecoration" {
            fields
                .get(14)
                .and_then(|field| parse_form_label_field_text_color(field, object_refs))
        } else {
            None
        },
        back_color: if let Some(schema) = button_color_schema {
            fields
                .get(schema.back_color_slot())
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if tag == "InputField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| options.get(schema.back_color_option_slot()?))
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if let Some(schema) = table_schema {
            fields
                .get(schema.back_color_slot())
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if let Some(value) = field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| options.get(schema.back_color_option_slot()?))
            .and_then(|field| parse_form_control_color(field, object_refs))
        {
            Some(value)
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.back_color.clone())
        } else if let Some(value) = show_title_schema
            .and_then(|schema| {
                show_title_options
                    .as_deref()?
                    .get(schema.back_color_option_slot()?)
            })
            .and_then(|field| parse_form_control_color(field, object_refs))
        {
            Some(value)
        } else {
            None
        },
        border_color: if let Some(schema) = button_color_schema {
            fields
                .get(schema.border_color_slot())
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if tag == "InputField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| options.get(schema.border_color_option_slot()?))
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if let Some(schema) = table_schema {
            fields
                .get(schema.border_color_slot())
                .and_then(|field| parse_form_control_color(field, object_refs))
        } else if let Some(value) = field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| options.get(schema.border_color_option_slot()?))
            .and_then(|field| parse_form_control_color(field, object_refs))
        {
            Some(value)
        } else {
            None
        },
        title_text_color: (tag == "UsualGroup")
            .then(|| parse_form_usual_group_title_text_color(&fields, object_refs))
            .flatten(),
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
        auto_insert_new_row: table_schema.and_then(|schema| schema.auto_insert_new_row(&fields)),
        format: if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .map(|options| options.format.clone())
                .unwrap_or_default()
        } else if tag == "LabelField" {
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
        title_font_xml: if tag == "UsualGroup" {
            parse_form_usual_group_title_font_xml(&fields, object_refs)
        } else if matches!(wrapper, "37" | "48")
            && matches!(
                tag,
                "InputField"
                    | "LabelField"
                    | "CheckBoxField"
                    | "PictureField"
                    | "RadioButtonField"
                    | "TextDocumentField"
                    | "ProgressBarField"
                    | "TrackBarField"
                    | "ChartField"
            )
        {
            fields
                .get(FieldSlot::TitleFont.index(input_field_top_level_offset))
                .and_then(|field| parse_form_title_font_tuple_xml(field, object_refs))
        } else {
            None
        },
        font_xml: if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.font_xml.clone())
        } else if tag == "LabelDecoration" && !title.is_empty() {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.font_xml.clone())
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_font_xml(input_field_extended_options.as_deref(), object_refs)
        } else if tag == "RadioButtonField" {
            parse_form_radio_button_font_xml(radio_button_options.as_deref(), object_refs)
        } else if let Some(field) = button_common_schema.and_then(|schema| schema.font(&fields)) {
            parse_form_button_font_tuple_xml(field, object_refs)
        } else {
            None
        },
        width: if let Some(schema) = command_bar_schema {
            schema.width(&fields)
        } else if tag == "Table" {
            table_schema.and_then(|schema| schema.width(&fields))
        } else if let Some(properties) = spreadsheet_document_properties.as_ref() {
            properties.width.clone()
        } else if tag == "CalendarField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(1))
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value != "16" && value.parse::<u32>().is_ok())
        } else if tag == "GraphicalSchemaField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(1))
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if tag == "UsualGroup" {
            parse_form_usual_group_width(&fields)
        } else if tag == "PictureField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| schema.width(options))
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_width(input_field_extended_options.as_deref())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.width.clone())
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.width().map(str::to_owned))
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.width().map(str::to_owned))
        } else if let Some((schema, options)) = special_field_layout.as_ref() {
            schema.width(options)
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(16 + button_top_level_offset)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else {
            None
        },
        height: if let Some(schema) = command_bar_schema {
            schema.height(&fields)
        } else if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .and_then(|options| options.height.clone())
        } else if let Some(properties) = spreadsheet_document_properties.as_ref() {
            properties.height.clone()
        } else if tag == "PictureField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| schema.height(options))
        } else if tag == "CalendarField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(2))
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value != "9" && value.parse::<u32>().is_ok())
        } else if matches!(tag, "GraphicalSchemaField" | "HTMLDocumentField") {
            let default_height = (tag == "HTMLDocumentField").then_some("10");
            document_field_options
                .as_deref()
                .and_then(|options| options.get(2))
                .map(|field| field.trim().to_string())
                .filter(|value| {
                    value != "0"
                        && default_height != Some(value.as_str())
                        && value.parse::<u32>().is_ok()
                })
        } else if tag == "TextDocumentField" && form_input_field_layout_is_extended(&fields) {
            fields
                .get(23)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if tag == "Table" {
            table_schema.and_then(|schema| schema.height(&fields))
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_height(input_field_extended_options.as_deref())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.height.clone())
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.height().map(str::to_owned))
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.height().map(str::to_owned))
        } else if tag == "Pages" {
            fields
                .get(13)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0")
        } else if let Some(schema) = button_common_schema {
            schema.height(&fields)
        } else {
            None
        },
        show_current_date: if tag == "CalendarField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(15))
                .and_then(|field| parse_form_child_item_show_title(field))
                .filter(|value| !*value)
        } else {
            None
        },
        show_months_panel: if tag == "CalendarField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(6))
                .and_then(|field| parse_form_child_item_show_title(field))
                .filter(|value| *value)
        } else {
            None
        },
        width_in_months: if tag == "CalendarField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(16))
                .map(|field| field.trim().to_string())
                .filter(|value| value != "1" && value.parse::<u32>().is_ok())
        } else {
            None
        },
        height_in_months: if tag == "CalendarField" {
            document_field_options
                .as_deref()
                .and_then(|options| options.get(17))
                .map(|field| field.trim().to_string())
                .filter(|value| value != "1" && value.parse::<u32>().is_ok())
        } else {
            None
        },
        auto_max_width: if let Some(properties) = spreadsheet_document_properties.as_ref() {
            properties.auto_max_width
        } else if matches!(tag, "InputField" | "TextDocumentField")
            && form_input_field_layout_is_extended(&fields)
        {
            parse_form_input_field_auto_max_width(input_field_extended_options.as_deref())
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            parse_form_button_auto_max_width(fields.get(34 + button_top_level_offset).copied())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.auto_max_width)
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.auto_max_width())
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.auto_max_width())
        } else if let Some((schema, options)) = special_field_layout.as_ref() {
            schema.auto_max_width(options)
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
            parse_form_button_max_width(fields.get(35 + button_top_level_offset).copied())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.max_width.clone())
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.max_width().map(str::to_owned))
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.max_width().map(str::to_owned))
        } else {
            None
        },
        auto_max_height: if let Some(properties) = spreadsheet_document_properties.as_ref() {
            properties.auto_max_height
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_auto_max_height(input_field_extended_options.as_deref())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.auto_max_height)
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.auto_max_height())
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.auto_max_height())
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
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.max_height().map(str::to_owned))
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.max_height().map(str::to_owned))
        } else {
            None
        },
        horizontal_stretch: if let Some(schema) = command_bar_schema {
            schema.horizontal_stretch(&fields)
        } else if let Some(schema) = button_common_schema {
            schema.horizontal_stretch(&fields)
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_horizontal_stretch(input_field_extended_options.as_deref())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.horizontal_stretch)
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.horizontal_stretch())
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.horizontal_stretch())
        } else if tag == "PictureField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| schema.horizontal_stretch(options))
        } else if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .and_then(|options| options.horizontal_stretch)
        } else if tag == "Page" {
            page_properties.and_then(|properties| properties.horizontal_stretch())
        } else if let Some((schema, options)) = special_field_layout.as_ref() {
            schema.horizontal_stretch(options)
        } else {
            None
        },
        vertical_stretch: if let Some(schema) = button_common_schema {
            schema.vertical_stretch(&fields)
        } else if let Some(properties) = spreadsheet_document_properties.as_ref() {
            properties.vertical_stretch
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_vertical_stretch(input_field_extended_options.as_deref())
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.geometry.vertical_stretch())
        } else if tag == "PictureDecoration" {
            picture_decoration_properties
                .as_ref()
                .and_then(|properties| properties.vertical_stretch())
        } else if tag == "PictureField" {
            field_schema_and_options
                .as_ref()
                .and_then(|(schema, options)| schema.vertical_stretch(options))
        } else if tag == "UsualGroup" {
            parse_form_usual_group_vertical_stretch(&fields)
        } else if tag == "Page" {
            page_properties.and_then(|properties| properties.vertical_stretch())
        } else {
            None
        },
        spreadsheet_document_properties,
        max_value: special_field_layout
            .as_ref()
            .and_then(|(schema, options)| schema.max_value(options)),
        input_min_value: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| {
                parse_form_input_field_decimal_option(*schema, options, InputFieldSlot::MinValue)
            }),
        input_max_value: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| {
                parse_form_input_field_decimal_option(*schema, options, InputFieldSlot::MaxValue)
            }),
        show_percent: special_field_layout
            .as_ref()
            .and_then(|(schema, options)| schema.show_percent(options)),
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
        extended_edit: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_extended_edit(input_field_extended_options.as_deref())
        } else {
            None
        },
        mask: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| parse_form_input_field_mask(*schema, options)),
        text_edit: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_text_edit(input_field_extended_options.as_deref())
        } else {
            None
        },
        edit_text_update: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| {
                parse_form_input_field_edit_text_update(*schema, options)
            }),
        auto_cell_height: field_schema_and_options
            .as_ref()
            .and_then(|(schema, _)| schema.auto_cell_height(&fields))
            .or_else(|| {
                (tag == "InputField" && form_input_field_layout_is_extended(&fields))
                    .then(|| parse_form_input_field_auto_cell_height(&fields))
                    .flatten()
            }),
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
        extended_edit_multiple_values: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| schema.extended_edit_multiple_values(options)),
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
        } else {
            None
        },
        incomplete_choice_mode: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| {
                parse_form_input_field_incomplete_choice_mode(*schema, options)
            }),
        choice_button_representation: if tag == "InputField"
            && form_input_field_layout_is_extended(&fields)
        {
            parse_form_input_field_choice_button_representation(
                input_field_extended_options.as_deref(),
            )
        } else {
            None
        },
        choice_button_picture_ref: choice_button_picture
            .as_ref()
            .and_then(|picture| picture.reference.clone()),
        choice_button_picture_load_transparent: choice_button_picture
            .as_ref()
            .is_some_and(|picture| picture.load_transparent),
        drop_list_width: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| parse_form_input_field_drop_list_width(*schema, options)),
        choice_history_on_input: field_schema_and_options
            .as_ref()
            .and_then(|(schema, options)| {
                parse_form_input_field_choice_history_on_input(*schema, options)
            }),
        item_type: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(4 + button_top_level_offset)
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
                .get(25 + button_top_level_offset)
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
        } else if tag == "Page" {
            page_picture
                .as_ref()
                .and_then(|picture| picture.reference.clone())
        } else {
            None
        },
        picture_load_transparent: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(25 + button_top_level_offset)
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
        } else if tag == "Page" {
            page_picture
                .as_ref()
                .is_some_and(|picture| picture.load_transparent)
        } else {
            false
        },
        header_picture_ref: header_picture
            .as_ref()
            .and_then(|picture| picture.reference.clone()),
        header_picture_file_name: header_picture
            .as_ref()
            .and_then(|picture| picture.file_name.clone()),
        header_picture_load_transparent: header_picture
            .as_ref()
            .is_some_and(|picture| picture.load_transparent),
        picture_size: if tag == "PictureDecoration" {
            parse_form_picture_decoration_picture_size(&fields)
        } else if tag == "PictureField" {
            parse_form_picture_field_picture_size(picture_field_options.as_deref())
        } else {
            None
        },
        picture_file_name: if tag == "PictureDecoration" {
            parse_form_picture_decoration_file_name(&fields)
        } else {
            None
        },
        title,
        usual_group_shortcut: extended_group_options
            .as_ref()
            .and_then(|options| options.shortcut.clone()),
        title_formatted,
        tooltip,
        input_hint,
        choice_list: if tag == "RadioButtonField" {
            parse_form_radio_button_choice_list(radio_button_options.as_deref(), object_refs)
        } else if tag == "InputField" {
            field_schema_and_options
                .as_ref()
                .map(|(schema, options)| {
                    parse_form_input_field_choice_list(*schema, options, object_refs)
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        },
        choice_parameter_links: parse_form_input_field_choice_parameter_links(
            audited_input_field_options,
            attribute_names_by_id,
        ),
        type_link: parse_form_input_field_type_link(
            audited_input_field_options,
            attribute_names_by_id,
            type_link_data_path_by_table_column,
        ),
        extended_tooltip: parse_form_child_item_extended_tooltip(&fields, object_refs),
        events: {
            let mut events = parse_form_child_item_event_fields(&fields);
            append_unique_form_body_events(
                &mut events,
                parse_form_schema_backed_child_item_events(
                    wrapper,
                    tag,
                    &fields,
                    direct_discriminator,
                    field_schema_and_options.as_ref(),
                ),
            );
            if tag == "InputField" {
                if let Some(extended_options) = input_field_extended_options.as_deref() {
                    append_unique_form_body_events(
                        &mut events,
                        parse_form_nested_child_item_event_records(extended_options),
                    );
                }
            }
            append_unique_form_body_events(
                &mut events,
                parse_form_html_document_field_option_events(
                    tag,
                    document_field_options.as_deref(),
                ),
            );
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
        data_path: data_path_resolution.map(|resolved| resolved.data_path),
        data_path_provenance,
        title_data_path: parse_form_title_data_path(
            tag,
            wrapper,
            &fields,
            conditional_group_schema.is_some(),
            attribute_names_by_id,
            attribute_metadata_owners_by_id,
            table_name_by_id,
            table_column_names_by_id,
            data_path_by_binding_key,
            object_refs,
        ),
        command_name,
        command_source: if tag == "CommandBar" {
            parse_form_command_bar_source_with_items(&fields, item_name_by_id)
        } else if tag == "ButtonGroup" {
            parse_form_button_group_command_source_with_items(&fields, item_name_by_id)
        } else if tag == "Popup" {
            parse_form_popup_command_source_with_items(&fields, item_name_by_id)
        } else {
            None
        },
        child_items,
    };
    if conditional_group_schema.is_some() {
        sanitize_form_conditional_group_descendants(&mut item.child_items);
    }
    Some(item)
}

fn sanitize_form_conditional_group_descendants(items: &mut [FormChildItem]) {
    for item in items {
        if item.data_path_provenance != Some(FormChildItemDataPathProvenance::DirectRawSlot) {
            item.data_path = None;
            item.data_path_provenance = None;
        }
        item.choice_parameter_links.clear();
        item.type_link = None;
        item.title_data_path = None;
        if item.tag == "LabelField" {
            item.width = None;
        }
        sanitize_form_conditional_group_descendants(&mut item.child_items);
    }
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
            | "FormattedDocumentField"
            | "CalendarField"
            | "GraphicalSchemaField"
            | "SpreadSheetDocumentField"
            | "HTMLDocumentField"
            | "ProgressBarField"
            | "TrackBarField"
            | "ChartField"
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
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    item_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
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
        attribute_metadata_owners_by_id,
        table_name_by_id,
        standard_command_owner_name_by_id,
        item_name_by_id,
        table_column_names_by_id,
        type_link_data_path_by_table_column,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        owner_scoped_bindings,
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
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    item_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) {
    for field in fields {
        let Some(item) = parse_form_child_item_with_metadata_owners(
            field,
            main_data_path,
            parent_data_path,
            parent_tag,
            attribute_names_by_id,
            attribute_metadata_owners_by_id,
            table_name_by_id,
            standard_command_owner_name_by_id,
            item_name_by_id,
            table_column_names_by_id,
            type_link_data_path_by_table_column,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            owner_scoped_bindings,
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
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
    item_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    if fields.get(41).map(|field| field.trim()) != Some("1") {
        return None;
    }
    parse_form_child_item_with_metadata_owners(
        fields.get(42)?,
        main_data_path,
        parent_data_path,
        parent_tag,
        attribute_names_by_id,
        attribute_metadata_owners_by_id,
        table_name_by_id,
        standard_command_owner_name_by_id,
        item_name_by_id,
        table_column_names_by_id,
        type_link_data_path_by_table_column,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        owner_scoped_bindings,
        commands,
        object_refs,
    )
}

pub(super) struct FormUsualGroupExtendedOptions {
    pub(super) group: Option<&'static str>,
    pub(super) behavior: Option<&'static str>,
    pub(super) representation: Option<&'static str>,
    pub(super) horizontal_stretch: Option<bool>,
    pub(super) enabled: Option<bool>,
    pub(super) read_only: Option<bool>,
    pub(super) height: Option<String>,
    pub(super) shortcut: Option<String>,
    pub(super) enable_content_change: Option<bool>,
    pub(super) group_horizontal_align: Option<&'static str>,
    pub(super) group_vertical_align: Option<FormUsualGroupGroupVerticalAlign>,
    pub(super) children_align: Option<&'static str>,
    pub(super) horizontal_spacing: Option<&'static str>,
    pub(super) vertical_spacing: Option<&'static str>,
    pub(super) child_items_width: Option<&'static str>,
    pub(super) control_representation: Option<&'static str>,
    pub(super) collapsed: Option<bool>,
    pub(super) collapsed_representation_title: Vec<(String, String)>,
    pub(super) horizontal_align: Option<&'static str>,
    pub(super) vertical_align: Option<&'static str>,
    pub(super) format: Vec<(String, String)>,
    pub(super) through_align: Option<&'static str>,
    pub(super) united: Option<bool>,
    pub(super) show_left_margin: Option<bool>,
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
    pub(super) auto_max_height: Option<bool>,
    pub(super) horizontal_stretch: Option<bool>,
    pub(super) format: Vec<(String, String)>,
    pub(super) font_xml: Option<String>,
    pub(super) text_color: Option<String>,
    pub(super) hyperlink_style: bool,
}

pub(super) struct FormLabelDecorationOptions {
    pub(super) hyperlink: bool,
    pub(super) font_xml: Option<String>,
    pub(super) text_color: Option<String>,
    pub(super) back_color: Option<String>,
    pub(super) group_horizontal_align: Option<&'static str>,
    pub(super) alignment: FormLabelDecorationAlignment,
    pub(super) geometry: FormLabelDecorationGeometry,
    pub(super) visual_tail: FormLabelDecorationVisualTail,
    pub(super) skip_on_input: Option<bool>,
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
        || (fields.get(55).map(|field| field.trim()) == Some(TableTailKey::RowFilter.key())
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
        .get(LabelFieldSlot::Width.index())
        .map(|field| field.trim())
        .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
        .map(str::to_string);
    let max_width = options
        .get(LabelFieldSlot::MaxWidth.index())
        .map(|field| field.trim())
        .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
        .map(str::to_string);
    let text_color = options
        .get(LabelFieldSlot::TextColor.index())
        .and_then(|field| parse_form_label_field_text_color(field, object_refs));
    let hyperlink_style = width.is_none() && max_width.is_some();
    Some(FormLabelFieldOptions {
        width: if hyperlink_style { None } else { width },
        height: if hyperlink_style {
            None
        } else {
            options
                .get(LabelFieldSlot::Height.index())
                .map(|field| field.trim())
                .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
                .map(str::to_string)
        },
        auto_max_width: if hyperlink_style {
            None
        } else {
            match options
                .get(LabelFieldSlot::AutoMaxWidth.index())
                .map(|field| field.trim())
            {
                Some("0") => Some(false),
                _ => None,
            }
        },
        max_width: if hyperlink_style { None } else { max_width },
        auto_max_height: if hyperlink_style {
            None
        } else {
            match options
                .get(LabelFieldSlot::AutoMaxHeight.index())
                .map(|field| field.trim())
            {
                Some("0") => Some(false),
                _ => None,
            }
        },
        horizontal_stretch: if hyperlink_style {
            None
        } else {
            match options
                .get(LabelFieldSlot::HorizontalStretch.index())
                .map(|field| field.trim())
            {
                Some("0") => Some(false),
                Some("1") => Some(true),
                _ => None,
            }
        },
        format: options
            .get(LabelFieldSlot::Format.index())
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        font_xml: options
            .get(LabelFieldSlot::Font.index())
            .and_then(|field| parse_form_font_tuple_xml(field, object_refs)),
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

pub(super) fn parse_form_control_color(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let color = split_1c_braced_fields(field.trim(), 0)?;
    if color.len() != 3 || color.first()?.trim() != "3" {
        return None;
    }
    let variant = color.get(1)?.trim();
    let payload = split_1c_braced_fields(color.get(2)?.trim(), 0)?;
    match variant {
        "0" if payload.len() == 1 => {
            let value = payload.first()?.trim().parse::<u32>().ok()? & 0x00ff_ffff;
            let red = value & 0xff;
            let green = (value >> 8) & 0xff;
            let blue = (value >> 16) & 0xff;
            Some(format!("#{red:02X}{green:02X}{blue:02X}"))
        }
        "2" if payload.len() == 1 => payload
            .first()?
            .trim()
            .parse::<i32>()
            .ok()
            .and_then(style_web_color_name)
            .map(ToOwned::to_owned),
        "3" if payload.len() == 1 => payload
            .first()?
            .trim()
            .parse::<i32>()
            .ok()
            .and_then(form_control_system_color_name)
            .map(ToOwned::to_owned),
        "3" if payload.len() == 2 && payload.first()?.trim() == "0" => {
            let uuid = parse_non_zero_uuid(payload.get(1)?.trim())?;
            object_refs
                .get(&uuid)
                .and_then(|reference| reference.strip_prefix("StyleItem."))
                .map(|name| format!("style:{name}"))
        }
        "4" if payload.len() == 1 && payload.first()?.trim() == "0" => None,
        _ => None,
    }
}

fn form_control_system_color_name(code: i32) -> Option<&'static str> {
    match code {
        -1 => Some("style:FormBackColor"),
        -3 => Some("style:FormTextColor"),
        -7 => Some("style:ButtonBackColor"),
        -10 => Some("style:FieldBackColor"),
        -11 => Some("style:FieldTextColor"),
        -14 => Some("style:FieldSelectionBackColor"),
        -16 => Some("style:SpecialTextColor"),
        -21 => Some("style:ButtonTextColor"),
        -22 => Some("style:BorderColor"),
        -23 => Some("style:ToolTipBackColor"),
        -37 => Some("style:TableFooterBackColor"),
        -46 => Some("style:AccentColor"),
        _ => None,
    }
}

pub(super) fn parse_form_label_decoration_options(
    item_tag: &str,
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormLabelDecorationOptions> {
    let options = split_1c_braced_fields(
        fields.get(FormLabelDecorationSchema::OPTIONS_SLOT)?.trim(),
        0,
    )?;
    let schema = FormLabelDecorationSchema::from_raw_layout(
        fields.first()?.trim(),
        fields.len(),
        item_tag,
        fields.get(5).map(|field| field.trim()),
        &options,
    )?;
    Some(FormLabelDecorationOptions {
        hyperlink: options.get(1).map(|field| field.trim()) == Some("1"),
        font_xml: fields
            .get(15)
            .and_then(|field| parse_form_font_tuple_xml(field, object_refs)),
        text_color: fields
            .get(14)
            .and_then(|field| parse_form_control_color(field, object_refs)),
        back_color: options
            .get(6)
            .and_then(|field| parse_form_control_color(field, object_refs)),
        group_horizontal_align: fields
            .get(schema.group_horizontal_align_slot())
            .and_then(|field| parse_form_label_decoration_group_horizontal_align(field)),
        alignment: schema.alignment(fields, &options),
        geometry: schema.geometry(fields),
        visual_tail: schema.visual_tail(&options),
        skip_on_input: schema.skip_on_input(fields),
    })
}

pub(super) fn parse_form_usual_group_title_text_color(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let color = split_1c_braced_fields(fields.get(16)?.trim(), 0)?;
    let reference = match color.first()?.trim() {
        "3" if color.len() == 3 && color.get(1)?.trim() == "3" => color.get(2)?.trim(),
        "4" if color.len() == 4 && color.get(1)?.trim() == "3" && color.get(3)?.trim() == "3" => {
            color.get(2)?.trim()
        }
        _ => return None,
    };
    let reference = split_1c_braced_fields(reference, 0)?;
    if reference.len() == 1 {
        let code = reference.first()?.trim().parse::<i32>().ok()?;
        return style_system_color_name(code).map(ToOwned::to_owned);
    }
    if reference.first()?.trim() == "0" {
        let uuid = parse_uuid_field(reference.get(1)?.trim())?;
        return object_refs
            .get(&uuid)
            .and_then(|reference| reference.strip_prefix("StyleItem."))
            .map(|name| format!("style:{name}"));
    }
    None
}

fn form_usual_group_title_font_fields<'a>(
    raw: &'a str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<&'a str>> {
    let font = split_1c_braced_fields(raw, 0)?;
    if !matches!(font.first()?.trim(), "7" | "8") || font.get(1)?.trim() != "2" {
        return None;
    }
    match font.len() {
        6 if font.get(2)?.trim() == "0"
            && font.get(4)?.trim() == "1"
            && font.get(5)?.trim() == "100" => {}
        10 if font.get(8)?.trim() == "1" => {}
        _ => return None,
    }
    style_body_ref_name(font.get(3)?.trim(), object_refs)?;
    Some(font)
}

pub(super) fn parse_form_usual_group_title_font_xml(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let raw = fields.get(17)?.trim();
    let font = form_usual_group_title_font_fields(raw, object_refs)?;
    let normalized = (font.first()?.trim() == "8").then(|| {
        let mut normalized = font.clone();
        normalized[0] = "7";
        format!("{{{}}}", normalized.join(","))
    });
    parse_form_title_font_tuple_xml(normalized.as_deref().unwrap_or(raw), object_refs)
}

pub(super) fn parse_form_usual_group_extended_options(
    fields: &[&str],
) -> Option<FormUsualGroupExtendedOptions> {
    let options =
        split_1c_braced_fields(fields.get(FormUsualGroupSchema::OPTIONS_SLOT)?.trim(), 0)?;
    match options.first()?.trim() {
        "29" => {
            let schema = FormUsualGroupSchema::from_raw_layout(
                fields.first()?.trim(),
                fields.len(),
                "UsualGroup",
                fields.get(5).map(|field| field.trim()),
                &options,
            )?;
            let properties = schema.properties(fields, &options);
            Some(FormUsualGroupExtendedOptions {
                group: parse_form_usual_group_property_bag_group(&options),
                behavior: parse_form_usual_group_property_bag_behavior(&options),
                representation: options
                    .get(3)
                    .and_then(|field| parse_form_child_item_representation(field)),
                horizontal_stretch: parse_form_usual_group_horizontal_stretch(fields),
                enabled: properties.enabled(),
                read_only: properties.read_only(),
                height: schema.height(fields),
                shortcut: schema
                    .shortcut_field(fields)
                    .and_then(|field| parse_common_command_shortcut_value(field)),
                enable_content_change: properties.enable_content_change(),
                group_horizontal_align: properties.group_horizontal_align(),
                group_vertical_align: properties.group_vertical_align(),
                children_align: properties.children_align(),
                horizontal_spacing: properties.horizontal_spacing(),
                vertical_spacing: properties.vertical_spacing(),
                child_items_width: properties.child_items_width(),
                control_representation: properties.control_representation(),
                collapsed: properties.collapsed(),
                collapsed_representation_title: schema
                    .collapsed_representation_title_field(&options)
                    .map(parse_form_localized_strings)
                    .unwrap_or_default(),
                horizontal_align: properties.horizontal_align(),
                vertical_align: properties.vertical_align(),
                format: schema
                    .format_field(&options)
                    .map(parse_form_localized_strings)
                    .unwrap_or_default(),
                through_align: properties.through_align(),
                united: properties.united(),
                show_left_margin: properties.show_left_margin(),
            })
        }
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
                enabled: None,
                read_only: None,
                height: None,
                shortcut: None,
                enable_content_change: None,
                group_horizontal_align: None,
                group_vertical_align: None,
                children_align: None,
                horizontal_spacing: None,
                vertical_spacing: None,
                child_items_width: None,
                control_representation: None,
                collapsed: None,
                collapsed_representation_title: Vec::new(),
                horizontal_align: None,
                vertical_align: None,
                format: Vec::new(),
                through_align: None,
                united: None,
                show_left_margin: None,
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
    options: &[&str],
) -> Option<&'static str> {
    match options.get(28).map(|value| value.trim())? {
        "0" => Some("Usual"),
        "1" => Some("Collapsible"),
        "2" => Some("PopUp"),
        "3" => None,
        _ => None,
    }
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

pub(super) fn form_button_layout_is_extended(fields: &[&str]) -> bool {
    fields.len() > 20
}

pub(super) fn form_button_top_level_offset(fields: &[&str]) -> usize {
    fields
        .get(5)
        .and_then(|field| parse_1c_quoted_string_with_len(field.trim()))
        .filter(|(value, _)| !value.is_empty())
        .map(|_| 0)
        .unwrap_or(1)
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct RawFormChoiceParameterLink {
    name: String,
    attribute_id: String,
    terminal: Option<&'static str>,
}

fn parse_exact_1c_quoted_string(field: &str) -> Option<String> {
    let field = field.trim();
    let (value, consumed) = parse_1c_quoted_string_with_len(field)?;
    (consumed == field.len()).then_some(value)
}

fn parse_raw_form_choice_parameter_link(
    field: &str,
    marker: &str,
    duplicate: bool,
) -> Option<RawFormChoiceParameterLink> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|field| field.trim()) != Some(marker)
        || fields.get(1).map(|field| field.trim()) != Some("1")
    {
        return None;
    }
    let name = parse_exact_1c_quoted_string(fields.get(2)?)?;
    if name.is_empty() {
        return None;
    }
    let mode = fields.get(3)?.trim();
    let duplicate_tail_len = if duplicate { 2 } else { 0 };
    let value_change_slot = match mode {
        "1" if fields.len() == 6 + duplicate_tail_len => 5,
        "2" if fields.len() == 7 + duplicate_tail_len => 6,
        _ => return None,
    };
    let owner = split_1c_braced_fields(fields.get(4)?.trim(), 0)?;
    if owner.len() != 1 {
        return None;
    }
    let attribute_id = owner.first()?.trim();
    if attribute_id.is_empty() {
        return None;
    }
    let terminal = if mode == "2" {
        let terminal = split_1c_braced_fields(fields.get(5)?.trim(), 0)?;
        if terminal.len() != 1 {
            return None;
        }
        match terminal.first()?.trim() {
            "-5" => Some("Owner"),
            "-8" => Some("Ref"),
            _ => return None,
        }
    } else {
        None
    };
    if fields.get(value_change_slot).map(|field| field.trim()) != Some("0") {
        return None;
    }
    if duplicate
        && (!fields[value_change_slot + 1..]
            .iter()
            .all(|field| parse_exact_1c_quoted_string(field).is_some_and(|value| value.is_empty()))
            || fields.len() != value_change_slot + 3)
    {
        return None;
    }
    Some(RawFormChoiceParameterLink {
        name,
        attribute_id: attribute_id.to_string(),
        terminal,
    })
}

pub(super) fn parse_form_input_field_choice_parameter_links(
    options: Option<&[&str]>,
    attribute_names_by_id: &BTreeMap<String, String>,
) -> Vec<FormChoiceParameterLink> {
    let Some(options) = options.filter(|options| {
        options.len() == 66 && options.first().map(|field| field.trim()) == Some("36")
    }) else {
        return Vec::new();
    };
    let Some(primary) = options
        .get(InputFieldSlot::ChoiceParameterLinks.index())
        .and_then(|field| parse_raw_form_choice_parameter_link(field, "5006", false))
    else {
        return Vec::new();
    };
    let Some(duplicate) = options
        .get(64)
        .and_then(|field| parse_raw_form_choice_parameter_link(field, "5007", true))
    else {
        return Vec::new();
    };
    if primary != duplicate {
        return Vec::new();
    }
    let Some(attribute_name) = attribute_names_by_id.get(&primary.attribute_id) else {
        return Vec::new();
    };
    let data_path = primary
        .terminal
        .map(|terminal| format!("{attribute_name}.{terminal}"))
        .unwrap_or_else(|| attribute_name.clone());
    vec![FormChoiceParameterLink {
        name: primary.name,
        data_path,
        value_change: "Clear",
    }]
}

pub(super) fn parse_form_input_field_type_link(
    options: Option<&[&str]>,
    attribute_names_by_id: &BTreeMap<String, String>,
    data_path_by_table_column: &BTreeMap<(String, String), String>,
) -> Option<FormTypeLink> {
    let options = options.filter(|options| {
        options.len() == 66 && options.first().map(|field| field.trim()) == Some("36")
    })?;
    let fields = options
        .get(InputFieldSlot::TypeLink.index())
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
    if fields.first().map(|field| field.trim()) != Some("3") {
        return None;
    }
    let (data_path, link_item) = match fields.as_slice() {
        [_, mode, owner, link_item] if mode.trim() == "1" && link_item.trim() == "0" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            if owner.len() != 1 {
                return None;
            }
            let data_path = attribute_names_by_id.get(owner.first()?.trim())?.clone();
            (data_path, "0")
        }
        [_, mode, owner, terminal, link_item] if mode.trim() == "2" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            if owner.len() != 2 || owner.get(1)?.trim() != FORM_ITEM_TYPE_UUID {
                return None;
            }
            let terminal = split_1c_braced_fields(terminal.trim(), 0)?;
            if terminal.len() != 1 {
                return None;
            }
            let link_item = match link_item.trim() {
                "0" => "0",
                "1" => "1",
                _ => return None,
            };
            let key = (
                owner.first()?.trim().to_string(),
                terminal.first()?.trim().to_string(),
            );
            (data_path_by_table_column.get(&key)?.clone(), link_item)
        }
        _ => return None,
    };
    Some(FormTypeLink {
        data_path,
        link_item,
    })
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

pub(super) fn parse_form_button_representation_in_context_menu(
    field: &str,
) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("None"),
        "1" => Some("AdditionalInContextMenu"),
        "2" => Some("OnlyInContextMenu"),
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
        Some("3") => Some("TabsOnLeftHorizontal"),
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

pub(super) fn parse_form_input_field_horizontal_align(fields: &[&str]) -> Option<&'static str> {
    let index = 23 + form_input_field_top_level_offset(fields);
    (fields.get(index)?.trim() == "2").then_some("Right")
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
        .and_then(|options| options.get(InputFieldSlot::Format.index()))
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default()
}

pub(super) fn parse_form_input_field_edit_format(
    extended_options: Option<&[&str]>,
) -> Vec<(String, String)> {
    extended_options
        .and_then(|options| options.get(InputFieldSlot::EditFormat.index()))
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default()
}

pub(super) fn parse_form_input_field_font_xml(
    extended_options: Option<&[&str]>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    extended_options
        .and_then(|options| options.get(InputFieldSlot::Font.index()))
        .and_then(|field| parse_form_font_tuple_xml(field, object_refs))
}

pub(super) fn parse_form_font_tuple_xml(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    parse_form_font_tuple_xml_tag(field, object_refs, "Font")
}

pub(super) fn parse_form_title_font_tuple_xml(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    parse_form_font_tuple_xml_tag(field, object_refs, "TitleFont")
}

pub(super) fn parse_form_font_tuple_xml_tag(
    field: &str,
    object_refs: &BTreeMap<String, String>,
    tag_name: &str,
) -> Option<String> {
    parse_form_font_tuple_xml_tag_with_absolute(field, object_refs, tag_name, false)
}

fn parse_form_button_font_tuple_xml(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let parsed = parse_form_font_tuple_xml_tag_with_absolute(field, object_refs, "Font", true);
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let style_relative = fields.len() == 8
        && fields.first().map(|value| value.trim()) == Some("7")
        && fields.get(1).map(|value| value.trim()) == Some("2")
        && fields.get(2).map(|value| value.trim()) == Some("3")
        && fields.get(6).map(|value| value.trim()) == Some("1")
        && fields.get(7).map(|value| value.trim()) == Some("100");
    if !style_relative {
        return parsed;
    }

    let Some(style_ref) = parse_form_font_style_ref(&fields, object_refs) else {
        return parsed;
    };
    let Some(height) = font_height_xml(fields.get(4).map(|value| value.trim())) else {
        return parsed;
    };
    let Some(face_name) = fields
        .get(5)
        .and_then(|value| parse_1c_quoted_string(value.trim()))
    else {
        return parsed;
    };
    Some(format!(
        r#"<Font ref="{}" faceName="{}" height="{}" kind="StyleItem"/>"#,
        escape_xml_text(&style_ref),
        escape_xml_text(&face_name),
        escape_xml_text(&height)
    ))
}

fn parse_form_font_tuple_xml_tag_with_absolute(
    field: &str,
    object_refs: &BTreeMap<String, String>,
    tag_name: &str,
    allow_absolute: bool,
) -> Option<String> {
    let trimmed = field.trim();
    let fields = split_1c_braced_fields(trimmed, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    match fields.get(1).map(|value| value.trim()) {
        Some("0") if allow_absolute => {}
        Some("1" | "2") => {}
        _ => return None,
    }
    let wrapped = format!("{{\"#\",00000000-0000-0000-0000-000000000000,1,{trimmed},0}}");
    let value_xml = parse_style_font_value_xml(&wrapped);
    let attrs = value_xml
        .strip_prefix(r#"<Value xsi:type="v8ui:Font""#)?
        .strip_suffix("/>")?;
    let attrs = if attrs.contains(" ref=") {
        attrs.to_string()
    } else if let Some(style_ref) = parse_form_font_style_ref(&fields, object_refs) {
        format!(r#" ref="{}"{attrs}"#, escape_xml_text(&style_ref))
    } else {
        attrs.to_string()
    };
    Some(format!("<{tag_name}{attrs}/>"))
}

pub(super) fn parse_form_font_style_ref(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if fields.get(1).map(|value| value.trim()) != Some("2") {
        return None;
    }
    let style_slot = split_1c_braced_fields(fields.get(3)?.trim(), 0)?;
    if style_slot.len() == 1 {
        let code = style_slot.first()?.trim().parse::<i32>().ok()?;
        return standard_style_item_for_code(code).map(|(_, name)| format!("style:{name}"));
    }
    if style_slot.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let uuid = parse_uuid_field(style_slot.get(1)?.trim())?;
    moxel_style_ref_for_uuid(&uuid, object_refs)
}

pub(super) fn parse_form_input_field_skip_on_input(field: &str) -> Option<bool> {
    match field.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_form_input_field_decimal_option(
    schema: FormFieldSchema,
    options: &[&str],
    slot: InputFieldSlot,
) -> Option<String> {
    let raw = schema.input_field_option(options, slot)?.trim();
    if scan_1c_braced_value(raw, 0) != Some(raw.len()) {
        return None;
    }
    let value = split_1c_braced_fields(raw, 0)?;
    match value.as_slice() {
        [kind, decimal]
            if kind.trim() == r#""N""# && information_register_decimal_is_valid(decimal.trim()) =>
        {
            Some(decimal.trim().to_string())
        }
        [kind] if kind.trim() == r#""U""# => None,
        _ => None,
    }
}

fn parse_form_input_field_mask(schema: FormFieldSchema, options: &[&str]) -> Option<String> {
    let mask = parse_exact_1c_quoted_string(
        schema
            .input_field_option(options, InputFieldSlot::Mask)?
            .trim(),
    )?;
    (!mask.is_empty()).then_some(mask)
}

fn parse_form_input_field_choice_button_picture(
    schema: FormFieldSchema,
    options: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormOwnedPicture> {
    let raw = schema
        .input_field_option(options, InputFieldSlot::ChoiceButtonPicture)?
        .trim();
    if scan_1c_braced_value(raw, 0) != Some(raw.len()) {
        return None;
    }
    let value = split_1c_braced_fields(raw, 0)?;
    let picture = schema.choice_button_picture(&value)?;
    parse_form_owned_picture(
        raw,
        &value,
        picture.kind(),
        picture.load_transparent(),
        "ChoiceButtonPicture",
        object_refs,
    )
}

fn parse_form_input_field_drop_list_width(
    schema: FormFieldSchema,
    options: &[&str],
) -> Option<String> {
    let value = schema
        .input_field_option(options, InputFieldSlot::DropListWidth)?
        .trim();
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value != 0)
        .map(|value| value.to_string())
}

fn parse_form_input_field_incomplete_choice_mode(
    schema: FormFieldSchema,
    options: &[&str],
) -> Option<&'static str> {
    match schema
        .input_field_option(options, InputFieldSlot::IncompleteChoiceMode)?
        .trim()
    {
        "0" => None,
        "1" => Some("OnActivate"),
        _ => None,
    }
}

fn parse_form_input_field_edit_text_update(
    schema: FormFieldSchema,
    options: &[&str],
) -> Option<&'static str> {
    match schema
        .input_field_option(options, InputFieldSlot::EditTextUpdate)?
        .trim()
    {
        "0" => None,
        "1" => Some("DontUse"),
        "2" => Some("OnValueChange"),
        _ => None,
    }
}

fn parse_form_input_field_choice_history_on_input(
    schema: FormFieldSchema,
    options: &[&str],
) -> Option<&'static str> {
    match schema
        .input_field_option(options, InputFieldSlot::ChoiceHistoryOnInput)?
        .trim()
    {
        "0" => None,
        "1" => Some("DontUse"),
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
    let value = extended_options?.get(InputFieldSlot::Width.index())?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_height(extended_options: Option<&[&str]>) -> Option<String> {
    let value = extended_options?
        .get(InputFieldSlot::Height.index())?
        .trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_auto_max_width(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::AutoMaxWidth.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_max_width(
    extended_options: Option<&[&str]>,
) -> Option<String> {
    let value = extended_options?
        .get(InputFieldSlot::MaxWidth.index())?
        .trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_auto_max_height(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::AutoMaxHeight.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_max_height(
    extended_options: Option<&[&str]>,
) -> Option<String> {
    let value = extended_options?
        .get(InputFieldSlot::MaxHeight.index())?
        .trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_horizontal_stretch(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::HorizontalStretch.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_vertical_stretch(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::VerticalStretch.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_password_mode(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::PasswordMode.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_multi_line(extended_options: Option<&[&str]>) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::MultiLine.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_wrap(extended_options: Option<&[&str]>) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::Wrap.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_extended_edit(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::ExtendedEdit.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        "2" => None,
        _ => None,
    }
}

pub(super) fn parse_form_input_field_text_edit(extended_options: Option<&[&str]>) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::TextEdit.index())
        .map(|field| field.trim())?
    {
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
    match extended_options?
        .get(InputFieldSlot::DropListButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_clear_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::ClearButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_open_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::OpenButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_create_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::CreateButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choice_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::ChoiceButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choice_list_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::ChoiceListButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_spin_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::SpinButton.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_list_choice_mode(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::ListChoiceMode.index())
        .map(|field| field.trim())?
    {
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_quick_choice(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::QuickChoice.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choose_type(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::ChooseType.index())
        .map(|field| field.trim())?
    {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_auto_choice_incomplete(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::AutoChoiceIncomplete.index())
        .map(|field| field.trim())?
    {
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_auto_mark_incomplete(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?
        .get(InputFieldSlot::AutoMarkIncomplete.index())
        .map(|field| field.trim())?
    {
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
    match extended_options?
        .get(InputFieldSlot::ChoiceButtonRepresentation.index())
        .map(|field| field.trim())?
    {
        "1" => Some("ShowInDropList"),
        "2" => Some("ShowInDropListAndInInputField"),
        "3" => Some("ShowInInputField"),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choice_folders_and_items(
    extended_options: Option<&[&str]>,
) -> Option<&'static str> {
    metadata_choice_folders_and_items_xml(
        extended_options?
            .get(InputFieldSlot::ChoiceFoldersAndItems.index())?
            .trim(),
    )
}

fn parse_form_check_box_field_layout<'a>(
    wrapper: &str,
    fields: &'a [&'a str],
) -> Option<(FormCheckBoxFieldSchema, Vec<&'a str>)> {
    let top_level_offset =
        FormCheckBoxFieldSchema::top_level_offset_for_raw_layout(wrapper, fields.len())?;
    let options = split_1c_braced_fields(fields.get(39 + top_level_offset)?.trim(), 0)?;
    let schema = FormCheckBoxFieldSchema::from_raw_layout(
        wrapper,
        fields.len(),
        fields.get(5 + top_level_offset).map(|field| field.trim()),
        &options,
    )?;
    debug_assert_eq!(schema.options_slot(), 39 + top_level_offset);
    Some((schema, options))
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
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    extended_options
        .and_then(|options| options.get(4))
        .and_then(|field| parse_form_font_tuple_xml(field, object_refs))
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
    schema: FormFieldSchema,
    options: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormChoiceListItem> {
    let Some(field) = schema.input_field_option(options, InputFieldSlot::ChoiceList) else {
        return Vec::new();
    };
    let field = field.trim();
    if scan_1c_braced_value(field, 0) != Some(field.len()) {
        return Vec::new();
    }
    let Some(fields) = split_1c_braced_fields(field, 0) else {
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
    let value = parse_form_input_field_choice_list_value(&payload_fields, object_refs)?;
    let presentation = parse_form_input_field_choice_list_presentation(payload_fields.get(5)?)?;
    Some(FormChoiceListItem {
        presentation_present: true,
        presentation,
        value,
    })
}

fn parse_form_input_field_choice_list_presentation(field: &str) -> Option<Vec<(String, String)>> {
    let field = field.trim();
    if scan_1c_braced_value(field, 0) != Some(field.len()) {
        return None;
    }
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let item_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != item_count.checked_add(2)? {
        return None;
    }
    fields
        .iter()
        .skip(2)
        .map(|item| {
            let item = item.trim();
            if scan_1c_braced_value(item, 0) != Some(item.len()) {
                return None;
            }
            let values = split_1c_braced_fields(item, 0)?;
            match values.as_slice() {
                [lang, content] => Some((
                    parse_exact_1c_quoted_string(lang.trim())?,
                    parse_exact_1c_quoted_string(content.trim())?,
                )),
                _ => None,
            }
        })
        .collect()
}

fn parse_form_input_field_choice_list_value(
    payload_fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChoiceListValue> {
    let raw_value = payload_fields.get(2)?.trim();
    if scan_1c_braced_value(raw_value, 0) != Some(raw_value.len()) {
        return None;
    }
    let value_fields = split_1c_braced_fields(raw_value, 0)?;
    match value_fields.as_slice() {
        [kind, value]
            if kind.trim() == r#""N""# && information_register_decimal_is_valid(value.trim()) =>
        {
            Some(FormChoiceListValue::Decimal(value.trim().to_string()))
        }
        [kind, value] if kind.trim() == r#""S""# => {
            parse_exact_1c_quoted_string(value.trim()).map(FormChoiceListValue::String)
        }
        [kind, value] if kind.trim() == r#""B""# => match value.trim() {
            "0" => Some(FormChoiceListValue::Boolean(false)),
            "1" => Some(FormChoiceListValue::Boolean(true)),
            _ => None,
        },
        [kind] if kind.trim() == r#""U""# => {
            let type_id = Uuid::parse_str(payload_fields.get(3)?.trim()).ok()?;
            let value_id = Uuid::parse_str(payload_fields.get(4)?.trim()).ok()?;
            if type_id.is_nil() && value_id.is_nil() {
                Some(FormChoiceListValue::Nil)
            } else {
                parse_design_time_reference(payload_fields.get(4)?.trim(), object_refs)
                    .map(FormChoiceListValue::DesignTimeRef)
            }
        }
        _ => None,
    }
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
        presentation_present: false,
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
        "0" => Some("List"),
        "2" => Some("Tree"),
        _ => None,
    }
}

pub(super) fn parse_form_table_wrapper73_representation(field: &str) -> Option<&'static str> {
    match field.trim() {
        "1" => Some("List"),
        _ => None,
    }
}

pub(super) fn parse_form_table_representation_from_fields(
    wrapper: &str,
    fields: &[&str],
) -> Option<&'static str> {
    match wrapper {
        "55" => fields
            .get(3)
            .and_then(|field| parse_form_table_representation(field)),
        "73" => fields
            .get(8)
            .and_then(|field| parse_form_table_wrapper73_representation(field)),
        _ => None,
    }
}

pub(super) fn parse_form_table_command_bar_location_field(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("None"),
        "2" => Some("Top"),
        "3" => Some("Bottom"),
        _ => None,
    }
}

pub(super) fn parse_form_table_command_bar_location(
    wrapper: &str,
    fields: &[&str],
) -> Option<&'static str> {
    match wrapper {
        "55" => fields
            .get(8)
            .and_then(|field| parse_form_table_command_bar_location_field(field)),
        _ => None,
    }
}

pub(super) fn form_table_wrapper55_uses_split_head_slots(fields: &[&str]) -> bool {
    fields.get(8).map(|field| field.trim()) == Some("2")
}

pub(super) fn parse_form_table_initial_tree_view(
    wrapper: &str,
    fields: &[&str],
) -> Option<&'static str> {
    match wrapper {
        "55" => fields.get(39).and_then(|field| match field.trim() {
            "1" => Some("ExpandTopLevel"),
            "2" => Some("ExpandAllLevels"),
            _ => None,
        }),
        _ => None,
    }
}

pub(super) fn parse_form_table_default_top_level_parent_nil(
    wrapper: &str,
    fields: &[&str],
) -> Option<bool> {
    if form_table_property_bag_value(fields, TableBagKey::TopLevelParent).is_some() {
        return None;
    }
    form_table_wrapper55_root_defaults(wrapper, fields).then_some(true)
}

pub(super) fn form_table_wrapper55_root_defaults(wrapper: &str, fields: &[&str]) -> bool {
    wrapper == "55"
        && fields.get(36).map(|field| field.trim()) == Some("1")
        && fields.get(37).map(|field| field.trim()) == Some("0")
}

pub(super) fn parse_form_table_row_selection_mode(field: &str) -> Option<&'static str> {
    match field.trim() {
        "1" => Some("Cell"),
        _ => None,
    }
}

pub(super) fn parse_form_table_file_drag_mode(field: &str) -> Option<&'static str> {
    match field.trim() {
        "2" => Some("AsFile"),
        _ => None,
    }
}

pub(super) fn parse_form_table_file_drag_mode_from_fields(
    wrapper: &str,
    fields: &[&str],
) -> Option<&'static str> {
    if wrapper == "55"
        && !form_table_wrapper55_uses_split_head_slots(fields)
        && fields.get(53).map(|field| field.trim()) == Some("0")
        && form_table_wrapper55_root_defaults(wrapper, fields)
    {
        return None;
    }
    fields
        .get(30)
        .and_then(|field| parse_form_table_file_drag_mode(field))
}

#[cfg(test)]
pub(super) fn parse_form_button_group_command_source(fields: &[&str]) -> Option<String> {
    parse_form_button_group_command_source_with_items(fields, &BTreeMap::new())
}

pub(super) fn parse_form_button_group_command_source_with_items(
    fields: &[&str],
    item_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let source = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    if source.len() != 4 {
        return None;
    }
    let form_ref = split_1c_braced_fields(source.get(1)?.trim(), 0)?;
    if form_ref.len() != 2 {
        return None;
    }
    match (
        source.first().map(|field| field.trim()),
        form_ref.get(1).map(|field| field.trim()),
        source.get(2).map(|field| field.trim()),
        source.get(3).map(|field| field.trim()),
    ) {
        (Some("2"), Some(FORM_ITEM_TYPE_UUID), Some("2"), Some("0")) => {
            form_command_source_name(form_ref.first()?.trim(), item_name_by_id)
        }
        (Some("2"), Some(FORM_GLOBAL_COMMAND_SOURCE_TYPE_UUID), Some("2"), Some("0"))
            if form_ref.first().map(|field| field.trim()) == Some("0") =>
        {
            Some("FormCommandPanelGlobalCommands".to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn parse_form_command_bar_source(fields: &[&str]) -> Option<String> {
    parse_form_command_bar_source_with_items(fields, &BTreeMap::new())
}

pub(super) fn parse_form_command_bar_source_with_items(
    fields: &[&str],
    item_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let source = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    if source.len() != 3 {
        return None;
    }
    let form_ref = split_1c_braced_fields(source.get(2)?.trim(), 0)?;
    if form_ref.len() != 2 {
        return None;
    }
    match (
        source.first().map(|field| field.trim()),
        form_ref.get(1).map(|field| field.trim()),
    ) {
        (Some("1"), Some(FORM_ITEM_TYPE_UUID)) => {
            form_command_source_name(form_ref.first()?.trim(), item_name_by_id)
        }
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn parse_form_popup_command_source(fields: &[&str]) -> Option<String> {
    parse_form_popup_command_source_with_items(fields, &BTreeMap::new())
}

pub(super) fn parse_form_popup_command_source_with_items(
    fields: &[&str],
    item_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let source = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    if source.len() != 9 {
        return None;
    }
    let source_ref = split_1c_braced_fields(source.get(2)?.trim(), 0)?;
    if source_ref.len() != 2 {
        return None;
    }
    if !matches!(
        (
            source.first().map(|field| field.trim()),
            source.get(3).map(|field| field.trim()),
            source.get(5).map(|field| field.trim()),
            source.get(6).map(|field| field.trim()),
        ),
        (Some("7"), Some("2"), Some("0"), Some("0"))
    ) {
        return None;
    }

    match (
        source_ref.first().map(|field| field.trim()),
        source_ref.get(1).map(|field| field.trim()),
    ) {
        (Some(source_id), Some(FORM_ITEM_TYPE_UUID)) => {
            form_command_source_name(source_id, item_name_by_id)
        }
        (Some("0"), Some(FORM_GLOBAL_COMMAND_SOURCE_TYPE_UUID)) => {
            Some("FormCommandPanelGlobalCommands".to_string())
        }
        _ => None,
    }
}

pub(super) fn form_command_source_name(
    item_id: &str,
    item_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    match item_id {
        "0" => Some("Form".to_string()),
        "-1" => Some("FormCommandPanelGlobalCommands".to_string()),
        _ => item_name_by_id
            .get(item_id)
            .map(|name| format!("Item.{name}")),
    }
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
struct FormTableRootProperties {
    restore_current_row: Option<bool>,
    top_level_parent_nil: Option<bool>,
    show_root: Option<bool>,
    allow_root_choice: Option<bool>,
}

fn parse_form_table_root_properties(
    schema: FormTableSchema,
    fields: &[&str],
) -> Option<FormTableRootProperties> {
    let (start, end) = schema.counted_property_bag_bounds(fields)?;
    let mut seen_keys = BTreeSet::new();
    let mut properties = FormTableRootProperties::default();

    for pair in fields.get(start..end)?.chunks_exact(2) {
        let raw_key = *pair.first()?;
        let key = raw_key.parse::<usize>().ok()?;
        if key.to_string() != raw_key {
            return None;
        }
        if !seen_keys.insert(key) {
            return None;
        }

        let value = pair.get(1)?.trim();
        if scan_1c_braced_value(value, 0) != Some(value.len()) {
            return None;
        }

        if key == TableRootBagKey::RestoreCurrentRow.key() {
            properties.restore_current_row = Some(parse_form_table_root_property_bool(value)?);
        } else if key == TableRootBagKey::TopLevelParent.key() {
            properties.top_level_parent_nil =
                Some(parse_form_table_root_property_undefined(value)?);
        } else if key == TableRootBagKey::ShowRoot.key() {
            properties.show_root = Some(parse_form_table_root_property_bool(value)?);
        } else if key == TableRootBagKey::AllowRootChoice.key() {
            properties.allow_root_choice = Some(parse_form_table_root_property_bool(value)?);
        }
    }

    Some(properties)
}

fn parse_form_table_root_property_bool(value: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.as_slice() {
        [marker, value] if marker.trim() == "\"B\"" => match value.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        },
        _ => None,
    }
}

fn parse_form_table_root_property_undefined(value: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(value, 0)?;
    matches!(fields.as_slice(), [marker] if marker.trim() == "\"U\"").then_some(true)
}

pub(super) fn parse_form_table_property_bag_bool(
    fields: &[&str],
    key: TableBagKey,
) -> Option<bool> {
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

pub(super) fn parse_form_table_property_bag_number(
    fields: &[&str],
    key: TableBagKey,
) -> Option<String> {
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

pub(super) fn parse_form_table_property_bag_string(
    fields: &[&str],
    key: TableBagKey,
) -> Option<String> {
    let value = form_table_property_bag_value(fields, key)?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.first().and_then(|field| parse_1c_string(field))? != "S" {
        return None;
    }
    fields.get(1).and_then(|field| parse_1c_string(field))
}

pub(super) fn parse_form_table_property_bag_undefined(
    fields: &[&str],
    key: TableBagKey,
) -> Option<bool> {
    let value = form_table_property_bag_value(fields, key)?;
    parse_form_standalone_undefined_marker(value)
}

pub(super) fn parse_form_table_period(fields: &[&str]) -> Option<FormTablePeriod> {
    let value = form_table_property_bag_value(fields, TableBagKey::Period)?;
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
    let value = form_table_property_bag_value(fields, TableBagKey::UpdateOnDataChange)?;
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
    let value = form_table_property_bag_value(fields, TableBagKey::ChoiceFoldersAndItems)?;
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

pub(super) fn form_table_property_bag_value<'a>(
    fields: &[&'a str],
    key: TableBagKey,
) -> Option<&'a str> {
    let key = key.key();
    fields.windows(2).find_map(|window| {
        (window[0].trim() == key && window[1].trim_start().starts_with('{')).then_some(window[1])
    })
}

fn form_table_excluded_command_name(_schema: FormTableSchema, uuid: &str) -> Option<&'static str> {
    match uuid {
        "11761e12-cf32-4826-a175-b23213e3b229" => Some("ChangeHistory"),
        "7d4db5ed-0981-4020-b3b8-886b7165ba05" => Some("SetPresentation"),
        "8af6ebff-cd02-4bfe-a984-44a292623708" => Some("ShowRowRearrangement"),
        "d96b0c03-b209-4d01-a3fc-17a14f873b64" => Some("SearchHistory"),
        "e6900951-1a42-4397-bf00-cabb2cd7ad6d" => Some("Detailed"),
        "ec576e13-1e76-4c33-98aa-a33204514227" => Some("Delete"),
        _ => form_table_standard_command_suffix(uuid),
    }
}

fn parse_form_table_counted_uuid_list(field: &str) -> Option<Vec<&str>> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if fields.len() != count.checked_add(1)? {
        return None;
    }
    let uuids: Vec<_> = fields.iter().skip(1).map(|uuid| uuid.trim()).collect();
    uuids
        .iter()
        .all(|uuid| parse_non_zero_uuid(uuid).is_some())
        .then_some(uuids)
}

fn form_table_event_collection_is_valid(field: &str) -> bool {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return false;
    };
    let Some(count) = fields
        .first()
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return false;
    };
    count
        .checked_mul(5)
        .and_then(|event_fields| event_fields.checked_add(3))
        == Some(fields.len())
}

fn form_table_child_owner_section_starts_at(fields: &[&str], index: usize) -> bool {
    fields.get(index).map(|field| field.trim()) == Some("1")
        && fields
            .get(index + 1)
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
            .and_then(|owner| owner.first().map(|field| field.trim() == "22"))
            == Some(true)
}

fn map_form_table_excluded_commands(
    schema: FormTableSchema,
    uuids: &[&str],
) -> Option<Vec<&'static str>> {
    let mut commands: Vec<_> = uuids
        .iter()
        .map(|uuid| form_table_excluded_command_name(schema, uuid))
        .collect::<Option<_>>()?;
    commands.sort_unstable();
    Some(commands)
}

fn parse_form_table_command_set_excluded_commands_for_table(
    schema: FormTableSchema,
    fields: &[&str],
) -> Vec<&'static str> {
    let pair_count_slot = schema.counted_property_bag_pair_count_slot();
    let Some(pair_count) = fields
        .get(pair_count_slot)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    let Some(event_slot) = pair_count
        .checked_mul(2)
        .and_then(|pair_fields| pair_count_slot.checked_add(1 + pair_fields))
    else {
        return Vec::new();
    };
    for pair_index in 0..pair_count {
        let key_slot = pair_count_slot + 1 + pair_index * 2;
        if fields
            .get(key_slot)
            .is_none_or(|field| field.trim().parse::<usize>().is_err())
            || fields
                .get(key_slot + 1)
                .and_then(|field| split_1c_braced_fields(field.trim(), 0))
                .is_none()
        {
            return Vec::new();
        }
    }
    if fields
        .get(event_slot)
        .is_none_or(|field| !form_table_event_collection_is_valid(field))
    {
        return Vec::new();
    }
    let command_slot = event_slot + 1;
    let Some(uuids) = fields
        .get(command_slot)
        .and_then(|field| parse_form_table_counted_uuid_list(field))
    else {
        return Vec::new();
    };
    if !form_table_child_owner_section_starts_at(fields, command_slot + 1) {
        return Vec::new();
    }
    map_form_table_excluded_commands(schema, &uuids).unwrap_or_default()
}

fn parse_form_field_command_set_excluded_commands(
    wrapper: &str,
    item_tag: &str,
    fields: &[&str],
) -> Vec<&'static str> {
    if wrapper != "37"
        || fields.len() != 59
        || fields.get(47).map(|field| field.trim()) != Some("\"\"")
        || fields.get(49).map(|field| field.trim()) != Some("0")
    {
        return Vec::new();
    }
    let mapper: fn(&str) -> Option<&'static str> =
        match (item_tag, fields.get(5).map(|field| field.trim())) {
            ("SpreadSheetDocumentField", Some("6")) => {
                form_spreadsheet_document_standard_command_suffix
            }
            ("FormattedDocumentField", Some("17")) => {
                form_formatted_document_standard_command_suffix
            }
            _ => return Vec::new(),
        };
    let Some(mut commands) = fields
        .get(48)
        .and_then(|field| parse_form_table_counted_uuid_list(field))
        .and_then(|uuids| {
            uuids
                .iter()
                .map(|uuid| mapper(uuid))
                .collect::<Option<Vec<_>>>()
        })
    else {
        return Vec::new();
    };
    commands.sort_unstable();
    commands
}

#[cfg(test)]
pub(super) fn parse_form_table_command_set_excluded_commands(fields: &[&str]) -> Vec<&'static str> {
    for field in fields {
        let Some(uuids) = parse_form_table_counted_uuid_list(field) else {
            continue;
        };
        if uuids.is_empty() {
            continue;
        }
        if let Some(commands) = map_form_table_excluded_commands(FormTableSchema, &uuids) {
            return commands;
        }
    }
    Vec::new()
}

fn parse_form_conditional_user_visible_common(field: &str) -> Option<bool> {
    let outer = split_1c_braced_fields(field.trim(), 0)?;
    if outer.len() != 2 || outer.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let condition = split_1c_braced_fields(outer.get(1)?.trim(), 0)?;
    if condition.len() != 3
        || condition.first().map(|value| value.trim()) != Some("0")
        || condition.get(2).map(|value| value.trim()) != Some("0")
    {
        return None;
    }
    let value = split_1c_braced_fields(condition.get(1)?.trim(), 0)?;
    if value.len() != 2 || value.first().and_then(|value| parse_1c_string(value))? != "B" {
        return None;
    }
    match value.get(1).map(|value| value.trim()) {
        Some("0") => Some(false),
        Some("1") => Some(true),
        _ => None,
    }
}

fn form_conditional_group_schema(
    wrapper: &str,
    fields: &[&str],
) -> Option<FormConditionalGroupSchema> {
    FormConditionalGroupSchema::from_raw_layout(
        wrapper,
        fields.len(),
        fields
            .get(5)
            .and_then(|field| parse_form_conditional_user_visible_common(field)),
        fields.get(6).map(|field| field.trim()),
    )
}

fn form_conditional_table_schema(
    wrapper: &str,
    fields: &[&str],
) -> Option<FormConditionalTableSchema> {
    FormConditionalTableSchema::from_raw_layout(
        wrapper,
        fields.len(),
        fields
            .get(5)
            .and_then(|field| parse_form_conditional_user_visible_common(field)),
        fields.get(4).map(|field| field.trim()),
    )
}

pub(super) fn form_child_item_tag(wrapper: &str, fields: &[&str]) -> Option<&'static str> {
    match wrapper {
        "22" => match fields
            .get(
                5 + form_conditional_group_schema(wrapper, fields)
                    .map(|_| 1)
                    .unwrap_or_default(),
            )
            .map(|value| value.trim())?
        {
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
                FormDecorationHeaderSchema::from_raw_layout(
                    wrapper,
                    fields.len(),
                    "LabelDecoration",
                    Some(kind),
                )
                .map(|_| "LabelDecoration")
            } else if kind == "1" {
                Some("PictureDecoration")
            } else {
                None
            }
        }
        "31" | "34" => Some("Button"),
        "37" | "48" => {
            if let Some((schema, _)) = parse_form_special_field_layout(wrapper, fields) {
                return Some(schema.xml_tag());
            }
            match fields
                .get(5 + form_input_field_top_level_offset(fields))
                .map(|value| value.trim())?
            {
                "1" => Some("LabelField"),
                "2" => Some("InputField"),
                "3" => Some("CheckBoxField"),
                "4" => Some("PictureField"),
                "5" => Some("RadioButtonField"),
                "6" => (wrapper == "37").then_some("SpreadSheetDocumentField"),
                "7" => Some("TextDocumentField"),
                "8" => (wrapper == "37").then_some("CalendarField"),
                "14" => (wrapper == "37").then_some("GraphicalSchemaField"),
                "15" => (wrapper == "37").then_some("HTMLDocumentField"),
                "17" => Some("FormattedDocumentField"),
                _ => None,
            }
        }
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

fn parse_form_special_field_layout<'a>(
    wrapper: &str,
    fields: &'a [&'a str],
) -> Option<(FormSpecialFieldSchema, Vec<&'a str>)> {
    let options = fields
        .get(FormSpecialFieldSchema::OPTIONS_SLOT)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
    let schema = FormSpecialFieldSchema::from_raw_layout(
        wrapper,
        fields.len(),
        fields.get(5).map(|field| field.trim()),
        options.len(),
        options.first().map(|field| field.trim()),
    )?;
    Some((schema, options))
}

pub(super) fn parse_form_child_item_name(wrapper: &str, fields: &[&str]) -> Option<String> {
    if wrapper == "22" {
        let index = 6 + form_conditional_group_schema(wrapper, fields)
            .map(|_| 1)
            .unwrap_or_default();
        return parse_1c_quoted_string_with_len(fields.get(index)?.trim())
            .map(|(value, _)| value)
            .filter(|value| !value.is_empty());
    }
    let indexes: &[usize] = match wrapper {
        "73" | "55" => &[5],
        "31" | "34" => &[5, 6],
        "37" | "48" => &[6, 7],
        _ => &[6],
    };
    indexes.iter().find_map(|index| {
        parse_1c_quoted_string_with_len(fields.get(*index)?.trim())
            .map(|(value, _)| value)
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn parse_form_child_item_title(
    tag: &str,
    wrapper: &str,
    fields: &[&str],
    field_schema: Option<FormFieldSchema>,
) -> (Vec<(String, String)>, Option<bool>) {
    if tag == "LabelDecoration"
        && let Some(title) = parse_form_label_decoration_title(fields)
    {
        return title;
    }
    if let Some(schema) = field_schema {
        return (
            fields
                .get(schema.title_slot())
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default(),
            None,
        );
    }
    let indexes: &[usize] = match (tag, wrapper) {
        ("FormattedDocumentField", "37" | "48") => &[9],
        (_, "73" | "55") => &[9],
        (_, "31" | "34") => &[6, 7],
        (_, "37" | "48") => &[9, 10],
        _ => &[7],
    };
    let values = indexes
        .iter()
        .find_map(|index| {
            let values = fields
                .get(*index)
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default();
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default();
    (values, None)
}

pub(super) fn parse_form_label_decoration_title(
    fields: &[&str],
) -> Option<(Vec<(String, String)>, Option<bool>)> {
    let options = split_1c_braced_fields(
        fields.get(FormLabelDecorationSchema::OPTIONS_SLOT)?.trim(),
        0,
    )?;
    let schema = FormLabelDecorationSchema::from_raw_layout(
        fields.first()?.trim(),
        fields.len(),
        "LabelDecoration",
        fields.get(5).map(|field| field.trim()),
        &options,
    )?;
    let title =
        split_1c_braced_fields(fields.get(FormLabelDecorationSchema::TITLE_SLOT)?.trim(), 0)?;
    let title_schema = schema.title_schema(&title)?;
    let values = title
        .get(title_schema.values_slot())
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default();
    let formatted = title
        .get(title_schema.formatted_slot())
        .map(|field| field.trim())
        == Some("1");
    let formatted = (formatted || !values.is_empty()).then_some(formatted);
    Some((values, formatted))
}

pub(super) fn parse_form_child_item_tooltip(
    tag: &str,
    wrapper: &str,
    fields: &[&str],
    field_schema: Option<FormFieldSchema>,
    check_box_schema: Option<FormCheckBoxFieldSchema>,
    table_schema: Option<FormTableSchema>,
) -> Vec<(String, String)> {
    if let Some(schema) = field_schema {
        return fields
            .get(schema.tooltip_slot())
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default();
    }
    if tag == "CheckBoxField" {
        return check_box_schema
            .and_then(|schema| fields.get(schema.tooltip_slot()))
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default();
    }
    if let Some(schema) = table_schema {
        return fields
            .get(schema.tooltip_slot())
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default();
    }
    let decoration_tooltip_slot = FormDecorationHeaderSchema::from_raw_layout(
        wrapper,
        fields.len(),
        tag,
        fields.get(5).map(|field| field.trim()),
    )
    .map(|schema| schema.tooltip_slot());
    let indexes: &[usize] = match wrapper {
        "22" => &[8],
        "37" | "48" => &[10, 11],
        _ => &[],
    };
    decoration_tooltip_slot
        .into_iter()
        .chain(indexes.iter().copied())
        .find_map(|index| {
            let values = fields
                .get(index)
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default();
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

pub(super) fn parse_form_field_tooltip_representation(
    wrapper: &str,
    tag: &str,
    fields: &[&str],
) -> Option<&'static str> {
    let schema = form_tooltip_representation_schema(
        wrapper,
        fields.len(),
        tag,
        fields.get(5).map(|field| field.trim()),
    )?;
    fields
        .get(schema.slot())
        .and_then(|field| decode_form_tooltip_representation(field.trim()))
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

fn parse_form_extended_tooltip_option_events(fields: &[&str]) -> Option<Vec<FormBodyEvent>> {
    if fields.first().map(|value| value.trim()) == Some("0") {
        return Some(Vec::new());
    }
    let events = parse_form_child_item_event_record(fields);
    (events.len() == 1 && events.first()?.name == "URLProcessing").then_some(events)
}

pub(super) fn parse_form_child_item_extended_tooltip(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormExtendedTooltip> {
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
        if !is_form_extended_tooltip_name(&name) {
            return None;
        }

        let mut tooltip = FormExtendedTooltip::new(name, id.to_string());
        let Some(options) = nested
            .get(FormExtendedTooltipSchema::OPTIONS_SLOT)
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
        else {
            return Some(tooltip);
        };
        let Some(title) = nested
            .get(FormExtendedTooltipSchema::TITLE_SLOT)
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
        else {
            return Some(tooltip);
        };
        let Some(event_fields) = options
            .get(FormExtendedTooltipSchema::EVENT_OPTION_SLOT)
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
        else {
            return Some(tooltip);
        };
        let Some(schema) = FormExtendedTooltipSchema::from_raw_layout(
            nested.first()?.trim(),
            nested.len(),
            nested.get(5).map(|field| field.trim()),
            &options,
            &title,
            &event_fields,
        ) else {
            return Some(tooltip);
        };
        let Some(events) = parse_form_extended_tooltip_option_events(&event_fields) else {
            return Some(tooltip);
        };
        tooltip.width = extract_form_dimension(&nested, schema.width_slot());
        tooltip.auto_max_width = match nested
            .get(schema.auto_max_width_slot())
            .map(|value| value.trim())
        {
            Some("0") => Some(false),
            _ => None,
        };
        tooltip.max_width = extract_form_dimension(&nested, schema.max_width_slot());
        tooltip.height = extract_form_dimension(&nested, schema.height_slot());
        tooltip.auto_max_height = match nested
            .get(schema.auto_max_height_slot())
            .map(|value| value.trim())
        {
            Some("0") => Some(false),
            _ => None,
        };
        tooltip.horizontal_stretch = match nested
            .get(schema.horizontal_stretch_slot())
            .map(|value| value.trim())
        {
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => None,
        };
        tooltip.vertical_stretch = match nested
            .get(schema.vertical_stretch_slot())
            .map(|value| value.trim())
        {
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => None,
        };
        tooltip.text_color = nested
            .get(schema.text_color_slot())
            .and_then(|field| parse_form_label_field_text_color(field, object_refs));
        tooltip.font_xml = nested
            .get(schema.font_slot())
            .and_then(|field| parse_form_font_tuple_xml(field, object_refs));
        let title_values =
            parse_form_localized_strings(title.get(schema.title_values_slot())?.trim());
        let title_formatted = title.get(schema.title_formatted_slot())?.trim() == "1";
        if title_formatted || !title_values.is_empty() {
            tooltip.title = Some(FormExtendedTooltipTitle {
                values: title_values,
                formatted: title_formatted,
            });
        }
        tooltip.group_horizontal_align = nested
            .get(schema.group_horizontal_align_slot())
            .and_then(|field| parse_form_button_group_horizontal_align(field));
        tooltip.vertical_align = match options
            .get(schema.vertical_align_option_slot())
            .map(|value| value.trim())
        {
            Some("0") => Some("Top"),
            Some("1") => Some("Center"),
            Some("2") => Some("Bottom"),
            _ => None,
        };
        tooltip.events = events;
        Some(tooltip)
    })
}

pub(super) fn parse_form_html_document_field_option_events(
    tag: &str,
    options: Option<&[&str]>,
) -> Vec<FormBodyEvent> {
    if tag != "HTMLDocumentField" {
        return Vec::new();
    }
    let Some(event_fields) = options
        .and_then(|options| options.get(5))
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    else {
        return Vec::new();
    };
    let mut events = parse_form_child_item_event_record(&event_fields);
    for event in &mut events {
        event.name = match event.name.as_str() {
            "Click" => "OnClick".to_string(),
            _ => event.name.clone(),
        };
    }
    events
}

pub(super) fn is_form_extended_tooltip_name(name: &str) -> bool {
    ["ExtendedTooltip", "РасширеннаяПодсказка"]
        .iter()
        .any(|marker| {
            let Some(marker_offset) = name.rfind(marker) else {
                return false;
            };
            name[marker_offset + marker.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        })
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormOwnedPicture {
    reference: Option<String>,
    file_name: Option<String>,
    load_transparent: bool,
}

fn parse_form_page_picture(
    schema: FormPageSchema,
    options: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormOwnedPicture> {
    let raw = options.get(schema.picture_option_slot())?.trim();
    let value = split_1c_braced_fields(raw, 0)?;
    let value_schema = schema.picture(&value)?;
    parse_form_owned_picture(
        raw,
        &value,
        value_schema.kind(),
        value_schema.load_transparent(),
        "Picture",
        object_refs,
    )
}

fn parse_form_field_header_picture(
    wrapper: &str,
    item_tag: &str,
    fields: &[&str],
    top_level_offset: usize,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormOwnedPicture> {
    let picture_slot = 29 + top_level_offset;
    let raw = fields.get(picture_slot)?.trim();
    let value = split_1c_braced_fields(raw, 0)?;
    let schema = FormFieldHeaderPictureSchema::from_raw_layout(
        wrapper,
        fields.len(),
        item_tag,
        top_level_offset,
        &value,
    )?;
    if schema.picture_slot() != picture_slot {
        return None;
    }
    parse_form_owned_picture(
        raw,
        &value,
        schema.kind(),
        schema.load_transparent(),
        "HeaderPicture",
        object_refs,
    )
}

fn parse_form_table_rows_picture(
    schema: FormTableSchema,
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormOwnedPicture> {
    let raw = fields.get(schema.rows_picture_slot())?.trim();
    let value = split_1c_braced_fields(raw, 0)?;
    let value_schema = schema.rows_picture(&value)?;
    parse_form_owned_picture(
        raw,
        &value,
        value_schema.kind(),
        value_schema.load_transparent(),
        "RowsPicture",
        object_refs,
    )
}

fn parse_form_owned_picture(
    raw: &str,
    value: &[&str],
    kind: FormPictureValueKind,
    expected_load_transparent: bool,
    property_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormOwnedPicture> {
    match kind {
        FormPictureValueKind::Empty => None,
        FormPictureValueKind::Reference => {
            let reference_fields = split_1c_braced_fields(value.get(2)?.trim(), 0)?;
            let exact_reference = match reference_fields.as_slice() {
                [code] => code.trim().parse::<i32>().is_ok_and(|code| code < 0),
                [kind, uuid] => kind.trim() == "0" && parse_non_zero_uuid(uuid.trim()).is_some(),
                _ => false,
            };
            if !exact_reference {
                return None;
            }
            let (reference, load_transparent) =
                parse_common_command_picture_value(raw, object_refs)?;
            if load_transparent != expected_load_transparent {
                return None;
            }
            Some(FormOwnedPicture {
                reference: Some(reference?),
                file_name: None,
                load_transparent,
            })
        }
        FormPictureValueKind::Embedded => {
            let payload = value
                .get(7)
                .and_then(|field| extract_base64_payload(field))?;
            let content = decode_base64_mime(payload)?;
            if !is_form_item_picture_content(&content) {
                return None;
            }
            Some(FormOwnedPicture {
                reference: None,
                file_name: Some(form_item_picture_file_name(property_name, &content)),
                load_transparent: expected_load_transparent,
            })
        }
    }
}

pub(super) fn parse_form_picture_field_value(
    options: Option<&[&str]>,
    object_refs: &BTreeMap<String, String>,
) -> Option<(String, bool)> {
    options
        .and_then(|options| options.get(5))
        .and_then(|field| parse_form_child_item_picture_value(field, object_refs))
}

pub(super) fn parse_form_picture_field_picture_size(
    options: Option<&[&str]>,
) -> Option<&'static str> {
    options
        .and_then(|options| options.get(8))
        .and_then(|field| field.trim().parse::<usize>().ok())
        .and_then(moxel_picture_size_mode)
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

fn parse_form_schema_backed_child_item_events(
    wrapper: &str,
    tag: &str,
    fields: &[&str],
    direct_discriminator: Option<&str>,
    field_schema_and_options: Option<&(FormFieldSchema, Vec<&str>)>,
) -> Vec<FormBodyEvent> {
    if let Some((field_schema, options)) = field_schema_and_options
        && let Some(schema) =
            FormChildItemEventCollectionSchema::from_field_schema(*field_schema, tag)
        && let Some(record) = options
            .get(schema.collection_slot())
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    {
        return parse_form_schema_backed_event_record(schema, &record);
    }

    if tag == "Pages"
        && let Some(container) = fields
            .get(20)
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
        && let Some(schema) = FormChildItemEventCollectionSchema::from_pages_layout(
            wrapper,
            fields.len(),
            tag,
            direct_discriminator,
            &container,
        )
        && let Some(record) = container
            .get(schema.collection_slot())
            .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    {
        return parse_form_schema_backed_event_record(schema, &record);
    }

    Vec::new()
}

fn parse_form_schema_backed_event_record(
    schema: FormChildItemEventCollectionSchema,
    fields: &[&str],
) -> Vec<FormBodyEvent> {
    let Some(count) = fields
        .first()
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    let Some(expected_fields) = count.checked_mul(5).and_then(|value| value.checked_add(3)) else {
        return Vec::new();
    };
    if fields.len() != expected_fields {
        return Vec::new();
    }

    let trailer_start = 1 + count * 2;
    if fields.get(trailer_start).map(|field| field.trim()) != Some("1")
        || fields.get(trailer_start + 1).map(|field| field.trim()) != Some("0")
    {
        return Vec::new();
    }

    let mut events = Vec::with_capacity(count);
    for event_index in 0..count {
        let event_id = fields[1 + event_index * 2].trim();
        let handler_field = fields[2 + event_index * 2].trim();
        let Some(name) = schema.event_name(event_id) else {
            return Vec::new();
        };
        let Some((handler, consumed)) = parse_1c_quoted_string_with_len(handler_field) else {
            return Vec::new();
        };
        let handler = handler.trim();
        if consumed != handler_field.len()
            || handler.is_empty()
            || !is_probable_form_event_handler(handler)
        {
            return Vec::new();
        }

        let metadata_start = trailer_start + 2 + event_index * 3;
        if !fields[metadata_start].trim().eq_ignore_ascii_case(event_id)
            || fields[metadata_start + 1].trim() != "0"
            || fields[metadata_start + 2].trim() != "1"
        {
            return Vec::new();
        }
        events.push(FormBodyEvent {
            name: name.to_string(),
            handler: handler.to_string(),
        });
    }
    events
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
    strict_field_data_path: bool,
    owner_scoped_data_path: bool,
    button_data_path_slot: Option<usize>,
    attribute_names_by_id: &BTreeMap<String, String>,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
    object_refs: &BTreeMap<String, String>,
) -> Option<ResolvedFormChildItemDataPath> {
    let owner_scoped_metadata = !matches!(tag, "ProgressBarField" | "TrackBarField" | "ChartField");
    let parse_direct_bound = |field: &&str| {
        let scoped = if strict_field_data_path {
            FormOwnerScopedDataPath::from_option(resolve_form_strict_field_model_data_path(
                field,
                attribute_metadata_owners_by_id,
                object_refs,
            ))
        } else {
            FormOwnerScopedDataPath::Unknown
        }
        .or_else(|| {
            if owner_scoped_data_path {
                resolve_form_owner_scoped_bound_data_path(
                    field,
                    attribute_metadata_owners_by_id,
                    owner_scoped_bindings,
                    object_refs,
                )
            } else {
                FormOwnerScopedDataPath::Unknown
            }
        });
        scoped
            .or_else(|| {
                let data_path = if owner_scoped_metadata {
                    parse_form_bound_data_path_with_metadata_owner(
                        field,
                        name,
                        attribute_names_by_id,
                        attribute_metadata_owners_by_id,
                        table_name_by_id,
                        table_column_names_by_id,
                        bound_table_path_by_binding_key,
                        table_column_names_by_binding_key,
                        object_refs,
                    )
                } else {
                    parse_form_bound_data_path(
                        field,
                        name,
                        attribute_names_by_id,
                        table_name_by_id,
                        table_column_names_by_id,
                        bound_table_path_by_binding_key,
                        table_column_names_by_binding_key,
                    )
                };
                FormOwnerScopedDataPath::from_option(data_path)
            })
            .with_provenance(FormChildItemDataPathProvenance::DirectRawSlot)
    };
    let parse_bound = |field: &&str| {
        parse_direct_bound(field).or_else(|| {
            let data_path = parse_form_bound_data_binding_key(field)
                .and_then(|binding_key| data_path_by_binding_key.get(&binding_key).cloned());
            FormChildItemDataPathResolution::from_option(
                data_path,
                FormChildItemDataPathProvenance::InferredFallback,
            )
        })
    };
    let resolve_slots =
        |slots: &[usize], resolver: &dyn Fn(&&str) -> FormChildItemDataPathResolution| {
            for slot in slots {
                if let Some(field) = fields.get(*slot) {
                    match resolver(field) {
                        FormChildItemDataPathResolution::Unknown => {}
                        resolved => return resolved,
                    }
                }
            }
            FormChildItemDataPathResolution::Unknown
        };
    let input_field_offset = matches!(
        tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
            | "FormattedDocumentField"
            | "CalendarField"
            | "GraphicalSchemaField"
            | "SpreadSheetDocumentField"
            | "HTMLDocumentField"
            | "ProgressBarField"
            | "TrackBarField"
            | "ChartField"
    )
    .then(|| {
        form_input_field_layout_is_extended(fields)
            .then(|| form_input_field_top_level_offset(fields))
    })
    .flatten()
    .unwrap_or(0);
    let input_slots = [11 + input_field_offset, 12 + input_field_offset];
    let data_path = match tag {
        "Table" => fields
            .get(11)
            .map(parse_bound)
            .unwrap_or(FormChildItemDataPathResolution::Unknown)
            .or_else(|| {
                FormChildItemDataPathResolution::from_option(
                    main_data_path.map(ToOwned::to_owned),
                    FormChildItemDataPathProvenance::InferredFallback,
                )
            }),
        "InputField"
        | "CheckBoxField"
        | "PictureField"
        | "RadioButtonField"
        | "FormattedDocumentField"
        | "ProgressBarField"
        | "TrackBarField"
        | "ChartField" => resolve_slots(&input_slots, &parse_bound).or_else(|| {
            FormChildItemDataPathResolution::from_option(
                parent_data_path.map(|parent| {
                    let name = normalize_form_data_path_child_name(parent, name);
                    format!("{parent}.{name}")
                }),
                FormChildItemDataPathProvenance::InferredFallback,
            )
        }),
        "CalendarField"
        | "GraphicalSchemaField"
        | "SpreadSheetDocumentField"
        | "HTMLDocumentField" => resolve_slots(&input_slots, &parse_bound),
        "LabelField" => resolve_slots(&input_slots, &parse_direct_bound),
        "TextDocumentField" => resolve_slots(&input_slots, &parse_bound),
        "Button" => button_data_path_slot
            .and_then(|slot| fields.get(slot))
            .map(|field| {
                resolve_form_owner_scoped_button_data_path(
                    field,
                    attribute_names_by_id,
                    table_name_by_id,
                    table_column_names_by_id,
                    type_link_data_path_by_table_column,
                )
                .with_provenance(FormChildItemDataPathProvenance::DirectRawSlot)
            })
            .unwrap_or(FormChildItemDataPathResolution::Unknown),
        _ => FormChildItemDataPathResolution::from_option(
            table_name_by_id.get(id).cloned(),
            FormChildItemDataPathProvenance::InferredFallback,
        ),
    };
    data_path.into_option()
}

pub(super) struct ResolvedFormChildItemDataPath {
    data_path: String,
    provenance: FormChildItemDataPathProvenance,
}

enum FormChildItemDataPathResolution {
    Unknown,
    Ambiguous,
    Resolved(ResolvedFormChildItemDataPath),
}

impl FormChildItemDataPathResolution {
    fn from_option(data_path: Option<String>, provenance: FormChildItemDataPathProvenance) -> Self {
        data_path
            .map(|data_path| {
                Self::Resolved(ResolvedFormChildItemDataPath {
                    data_path,
                    provenance,
                })
            })
            .unwrap_or(Self::Unknown)
    }

    fn or_else(self, fallback: impl FnOnce() -> Self) -> Self {
        match self {
            Self::Unknown => fallback(),
            resolved => resolved,
        }
    }

    fn into_option(self) -> Option<ResolvedFormChildItemDataPath> {
        match self {
            Self::Resolved(resolved) => Some(resolved),
            Self::Unknown | Self::Ambiguous => None,
        }
    }
}

enum FormOwnerScopedDataPath {
    Unknown,
    Ambiguous,
    Resolved(String),
}

impl FormOwnerScopedDataPath {
    fn from_option(data_path: Option<String>) -> Self {
        data_path.map(Self::Resolved).unwrap_or(Self::Unknown)
    }

    fn or_else(self, fallback: impl FnOnce() -> Self) -> Self {
        match self {
            Self::Unknown => fallback(),
            resolved => resolved,
        }
    }

    fn with_provenance(
        self,
        provenance: FormChildItemDataPathProvenance,
    ) -> FormChildItemDataPathResolution {
        match self {
            Self::Unknown => FormChildItemDataPathResolution::Unknown,
            Self::Ambiguous => FormChildItemDataPathResolution::Ambiguous,
            Self::Resolved(data_path) => {
                FormChildItemDataPathResolution::Resolved(ResolvedFormChildItemDataPath {
                    data_path,
                    provenance,
                })
            }
        }
    }
}

fn resolve_form_owner_scoped_button_data_path(
    field: &str,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
) -> FormOwnerScopedDataPath {
    let no_global_binding_paths = BTreeMap::new();
    FormOwnerScopedDataPath::from_option(parse_form_button_data_path(
        field,
        attribute_names_by_id,
        table_name_by_id,
        table_column_names_by_id,
        type_link_data_path_by_table_column,
        &no_global_binding_paths,
    ))
}

fn resolve_form_owner_scoped_bound_data_path(
    field: &str,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
    object_refs: &BTreeMap<String, String>,
) -> FormOwnerScopedDataPath {
    let attribute_column = resolve_form_attribute_column_data_path(
        field,
        attribute_metadata_owners_by_id,
        owner_scoped_bindings,
    );
    if !matches!(attribute_column, FormOwnerScopedDataPath::Unknown) {
        return attribute_column;
    }
    if let Some(data_path) = resolve_form_owner_scoped_metadata_data_path(
        field,
        attribute_metadata_owners_by_id,
        object_refs,
    ) {
        return FormOwnerScopedDataPath::Resolved(data_path);
    }
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let Some(owner) = fields
        .get(1)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))
    else {
        return FormOwnerScopedDataPath::Unknown;
    };
    if owner.len() != 1 {
        return FormOwnerScopedDataPath::Unknown;
    }
    let attribute_id = owner[0].trim().to_string();
    let Some(table_key) = fields
        .get(2)
        .and_then(|field| parse_form_binding_key(field.trim()))
    else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let table_lookup = FormBoundTableKey {
        attribute_id: attribute_id.clone(),
        table_key: table_key.clone(),
    };
    let Some(table_path) = owner_scoped_bindings.table_paths.get(&table_lookup) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let Some(table_path) = table_path.as_ref() else {
        return FormOwnerScopedDataPath::Ambiguous;
    };
    if fields.first().map(|field| field.trim()) == Some("2") {
        return FormOwnerScopedDataPath::Resolved(table_path.clone());
    }
    if fields.first().map(|field| field.trim()) != Some("3") {
        return FormOwnerScopedDataPath::Unknown;
    }
    let Some(column_key) = fields
        .get(3)
        .and_then(|field| parse_form_binding_key(field.trim()))
    else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let column_lookup = FormBoundColumnKey {
        attribute_id,
        table_key,
        column_key,
    };
    match owner_scoped_bindings.column_names.get(&column_lookup) {
        Some(Some(column_name)) => {
            let column_name = normalize_form_table_column_name(table_path, column_name);
            FormOwnerScopedDataPath::Resolved(format!("{table_path}.{column_name}"))
        }
        Some(None) => FormOwnerScopedDataPath::Ambiguous,
        None => FormOwnerScopedDataPath::Unknown,
    }
}

fn resolve_form_attribute_column_data_path(
    field: &str,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    owner_scoped_bindings: &FormOwnerScopedBindingIndexes,
) -> FormOwnerScopedDataPath {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let [kind, owner, column] = fields.as_slice() else {
        return FormOwnerScopedDataPath::Unknown;
    };
    if kind.trim() != "2" {
        return FormOwnerScopedDataPath::Unknown;
    }
    let Some(owner) = split_1c_braced_fields(owner.trim(), 0) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let Some(column) = split_1c_braced_fields(column.trim(), 0) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    if owner.len() != 1 || column.len() != 1 {
        return FormOwnerScopedDataPath::Unknown;
    }
    let attribute_id = owner[0].trim();
    let column_id = column[0].trim();
    let Ok(column_number) = column_id.parse::<u64>() else {
        return FormOwnerScopedDataPath::Unknown;
    };
    if column_number == 0 || column_number.to_string() != column_id {
        return FormOwnerScopedDataPath::Unknown;
    }
    let key = FormAttributeColumnKey {
        attribute_id: attribute_id.to_string(),
        column_id: column_id.to_string(),
    };
    let Some(column_name) = owner_scoped_bindings.attribute_columns.get(&key) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    let Some(column_name) = column_name else {
        return FormOwnerScopedDataPath::Ambiguous;
    };
    let Some(attribute) = attribute_metadata_owners_by_id.get(attribute_id) else {
        return FormOwnerScopedDataPath::Unknown;
    };
    FormOwnerScopedDataPath::Resolved(format!("{}.{}", attribute.name, column_name))
}

fn parse_form_bound_data_path_with_metadata_owner(
    field: &str,
    name: &str,
    attribute_names_by_id: &BTreeMap<String, String>,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    resolve_form_owner_scoped_metadata_data_path(
        field,
        attribute_metadata_owners_by_id,
        object_refs,
    )
    .or_else(|| {
        parse_form_bound_data_path(
            field,
            name,
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
        )
    })
}

fn resolve_form_owner_scoped_metadata_data_path(
    field: &str,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let (owner_field, terminal_field) = match fields.as_slice() {
        [kind, owner, terminal] if kind.trim() == "2" => (*owner, *terminal),
        [kind, owner, table, terminal]
            if kind.trim() == "3" && parse_form_binding_key(table.trim()).is_some() =>
        {
            (*owner, *terminal)
        }
        _ => return None,
    };
    let owner = split_1c_braced_fields(owner_field.trim(), 0)?;
    if owner.len() != 1 {
        return None;
    }
    let attribute = attribute_metadata_owners_by_id.get(owner.first()?.trim())?;

    let terminal = split_1c_braced_fields(terminal_field.trim(), 0)?;
    if terminal.len() != 2 || terminal.first()?.trim().parse::<i64>().is_err() {
        return None;
    }
    let uuid = parse_non_zero_uuid(terminal.get(1)?.trim())?;
    let reference = object_refs.get(&uuid)?;
    let (owner_base, relative_path) = form_metadata_data_path_route(reference)?;
    if !form_attribute_matches_metadata_owner(attribute, &owner_base) {
        return None;
    }
    Some(format!("{}.{}", attribute.name, relative_path))
}

fn resolve_form_strict_field_model_data_path(
    field: &str,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let [kind, owner, terminal] = fields.as_slice() else {
        return None;
    };
    if kind.trim() != "2" {
        return None;
    }
    let owner = split_1c_braced_fields(owner.trim(), 0)?;
    if owner.len() != 1 {
        return None;
    }
    let attribute = attribute_metadata_owners_by_id.get(owner.first()?.trim())?;
    let terminal = split_1c_braced_fields(terminal.trim(), 0)?;
    match terminal.as_slice() {
        [marker] => {
            let marker = marker.trim();
            marker.parse::<i64>().ok()?;
            resolve_form_owner_scoped_standard_attribute_data_path(attribute, marker)
        }
        [marker, uuid] => {
            marker.trim().parse::<i64>().ok()?;
            let uuid = parse_non_zero_uuid(uuid.trim())?;
            let reference = object_refs.get(&uuid)?;
            resolve_form_constants_set_data_path(attribute, reference)
        }
        _ => None,
    }
}

fn resolve_form_owner_scoped_standard_attribute_data_path(
    attribute: &FormAttributeMetadataOwner,
    marker: &str,
) -> Option<String> {
    let reference = attribute.exact_single_type_reference.as_deref()?;
    let (generated_type, _) = form_generated_owner_type_from_type_reference(reference)?;
    let attribute_name = match generated_type {
        "ChartOfAccountsObject" => chart_of_accounts_standard_attribute_name(marker),
        _ => None,
    }?;
    Some(format!("{}.{}", attribute.name, attribute_name))
}

fn resolve_form_constants_set_data_path(
    attribute: &FormAttributeMetadataOwner,
    reference: &str,
) -> Option<String> {
    if attribute.exact_single_type_reference.as_deref() != Some("cfg:ConstantsSet") {
        return None;
    }
    let (kind, constant_name) = reference.split_once('.')?;
    if kind != "Constant" || constant_name.is_empty() || constant_name.contains('.') {
        return None;
    }
    Some(format!("{}.{}", attribute.name, constant_name))
}

fn form_metadata_data_path_route(reference: &str) -> Option<(String, String)> {
    let mut parts = reference.split('.');
    let owner_kind = parts.next()?;
    let owner_name = parts.next()?;
    if owner_kind.is_empty() || owner_name.is_empty() {
        return None;
    }
    let owner_base = format!("{owner_kind}.{owner_name}");
    let relative = reference.strip_prefix(&format!("{owner_base}."))?;
    let route = relative.split('.').collect::<Vec<_>>();
    let relative_path = match route.as_slice() {
        ["Attribute" | "Dimension" | "Resource", name] if !name.is_empty() => (*name).to_string(),
        ["TabularSection", table, "Attribute", name] if !table.is_empty() && !name.is_empty() => {
            format!("{table}.{name}")
        }
        _ => return None,
    };
    Some((owner_base, relative_path))
}

fn form_attribute_matches_metadata_owner(
    attribute: &FormAttributeMetadataOwner,
    owner_base: &str,
) -> bool {
    let mut proven_bases = attribute
        .type_references
        .iter()
        .filter_map(|reference| form_metadata_owner_base_from_type_reference(reference))
        .collect::<BTreeSet<_>>();
    if let Some(main_table) = attribute.main_table.as_ref() {
        proven_bases.insert(main_table.clone());
    }
    proven_bases.len() == 1 && proven_bases.contains(owner_base)
}

pub(super) fn parse_form_title_data_path(
    tag: &str,
    wrapper: &str,
    fields: &[&str],
    conditional_layout: bool,
    attribute_names_by_id: &BTreeMap<String, String>,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if wrapper != "22" || conditional_layout {
        return None;
    }
    let (options_kind, options_len, binding_slot) = match tag {
        "Page" if matches!(fields.len(), 32 | 34 | 40) => ("18", 20, 4),
        "UsualGroup" if matches!(fields.len(), 32 | 34) => ("29", 29, 5),
        _ => return None,
    };
    let options = fields
        .get(20)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
    if options.len() != options_len
        || options.first().map(|field| field.trim()) != Some(options_kind)
    {
        return None;
    }
    let binding = options
        .get(binding_slot)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
    match binding.as_slice() {
        [kind, owner] if kind.trim() == "1" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            if owner.len() != 1 {
                return None;
            }
            attribute_names_by_id.get(owner.first()?.trim()).cloned()
        }
        [kind, owner, metadata, terminal] if tag == "Page" && kind.trim() == "3" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            let metadata = split_1c_braced_fields(metadata.trim(), 0)?;
            let terminal = split_1c_braced_fields(terminal.trim(), 0)?;
            if owner.len() != 1
                || metadata.len() != 2
                || metadata.first()?.trim() != "0"
                || terminal.len() != 1
                || terminal.first()?.trim() != "100000000"
            {
                return None;
            }
            let uuid = parse_non_zero_uuid(metadata.get(1)?.trim())?;
            resolve_form_title_rows_count_path(
                owner.first()?.trim(),
                &uuid,
                attribute_metadata_owners_by_id,
                object_refs,
            )
        }
        [kind, owner, terminal] if tag == "UsualGroup" && kind.trim() == "2" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            let terminal = split_1c_braced_fields(terminal.trim(), 0)?;
            if owner.len() != 2
                || owner.get(1)?.trim() != FORM_ITEM_TYPE_UUID
                || terminal.len() != 1
            {
                return None;
            }
            resolve_form_item_current_data_path(
                owner.first()?.trim(),
                terminal.first()?.trim(),
                table_name_by_id,
                table_column_names_by_id,
                data_path_by_binding_key,
            )
        }
        _ => None,
    }
}

fn resolve_form_title_rows_count_path(
    attribute_id: &str,
    uuid: &str,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let attribute = attribute_metadata_owners_by_id.get(attribute_id)?;
    let reference = object_refs.get(uuid)?;
    let route = reference.split('.').collect::<Vec<_>>();
    let (owner_kind, owner_name, table_name) = match route.as_slice() {
        [owner_kind, owner_name, "TabularSection", table_name]
            if !owner_kind.is_empty() && !owner_name.is_empty() && !table_name.is_empty() =>
        {
            (*owner_kind, *owner_name, *table_name)
        }
        _ => return None,
    };
    let owner_base = format!("{owner_kind}.{owner_name}");
    if !form_attribute_matches_metadata_owner(attribute, &owner_base) {
        return None;
    }
    Some(format!("{}.{}.RowsCount", attribute.name, table_name))
}

fn form_metadata_owner_base_from_type_reference(reference: &str) -> Option<String> {
    let (generated_type, owner_name) = form_generated_owner_type_from_type_reference(reference)?;
    let owner_kind = [
        "RecordManager",
        "RecordSet",
        "RecordKey",
        "Object",
        "Record",
        "Ref",
    ]
    .into_iter()
    .find_map(|role| generated_type.strip_suffix(role))
    .filter(|owner_kind| !owner_kind.is_empty())?;
    Some(format!("{owner_kind}.{owner_name}"))
}

fn form_generated_owner_type_from_type_reference(reference: &str) -> Option<(&str, &str)> {
    let reference = reference.strip_prefix("cfg:")?;
    let (generated_type, owner_name) = reference.split_once('.')?;
    if owner_name.is_empty() || owner_name.contains('.') {
        return None;
    }
    Some((generated_type, owner_name))
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

pub(super) fn parse_form_table_row_picture_data_path(
    schema: FormTableSchema,
    fields: &[&str],
    data_path: Option<&str>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    attribute_metadata_owners_by_id: &BTreeMap<String, FormAttributeMetadataOwner>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let table_name = data_path?;
    let encoded =
        split_1c_braced_fields(fields.get(schema.row_picture_data_path_slot())?.trim(), 0)?;
    let payload = match schema.row_picture_data_path(&encoded)? {
        FormTableRowPictureDataPath::Empty => return None,
        FormTableRowPictureDataPath::Payload(payload) => payload,
    };
    let payload = split_1c_braced_fields(payload, 0)?;
    let column_name = match payload.as_slice() {
        [column_id] if column_id.trim() == "10000000" => {
            let hidden =
                parse_exact_form_attribute_binding_id(fields.get(schema.data_path_slot())?.trim())
                    .and_then(|attribute_id| attribute_metadata_owners_by_id.get(&attribute_id))
                    .is_some_and(|attribute| {
                        attribute.has_dynamic_list_settings && attribute.main_table.is_none()
                    });
            let prefix = if hidden && !table_name.starts_with('~') {
                "~"
            } else {
                ""
            };
            return Some(format!("{prefix}{table_name}.DefaultPicture"));
        }
        [column_id] if column_id.trim().parse::<u64>().is_ok() => {
            let table_id =
                parse_exact_form_attribute_binding_id(fields.get(schema.data_path_slot())?.trim())?;
            table_column_names_by_id
                .get(&table_id)?
                .get(column_id.trim())?
                .as_str()
        }
        [kind, uuid] if matches!(kind.trim(), "0" | "4") => {
            let uuid = parse_non_zero_uuid(uuid.trim())?;
            let reference = object_refs.get(&uuid)?;
            form_metadata_attribute_suffix(reference)?
        }
        _ => return None,
    };
    let column_name = normalize_form_table_column_name(table_name, column_name);
    Some(format!("{table_name}.{column_name}"))
}

fn form_metadata_attribute_suffix(reference: &str) -> Option<&str> {
    let mut parts = reference.rsplit('.');
    let name = parts.next()?;
    let kind = parts.next()?;
    (kind == "Attribute" && !name.is_empty()).then_some(name)
}

pub(super) fn normalize_form_table_column_name(table_name: &str, field_name: &str) -> String {
    let field_name = [Some(table_name), table_name.rsplit('.').next()]
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
        .unwrap_or_else(|| field_name.to_string());
    normalize_form_data_path_child_name(table_name, &field_name)
}

pub(super) fn normalize_form_data_path_child_name(parent_path: &str, name: &str) -> String {
    if form_data_path_uses_standard_property_names(parent_path) {
        normalize_form_standard_data_path_name(name)
    } else {
        name.to_string()
    }
}

pub(super) fn form_data_path_uses_standard_property_names(path: &str) -> bool {
    matches!(path.split('.').next().unwrap_or(path), "Объект" | "Запись")
}

pub(super) fn normalize_form_standard_data_path_name(name: &str) -> String {
    FORM_STANDARD_DATA_PATH_NAME_ALIASES
        .iter()
        .find_map(|(source, target)| (*source == name).then_some((*target).to_string()))
        .unwrap_or_else(|| name.to_string())
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
            | "CalendarField"
            | "GraphicalSchemaField"
            | "SpreadSheetDocumentField"
            | "HTMLDocumentField"
    )
    .then(|| {
        form_input_field_layout_is_extended(fields)
            .then(|| form_input_field_top_level_offset(fields))
    })
    .flatten()
    .unwrap_or(0);
    match tag {
        "Table" => fields.get(11).copied().into_iter().collect(),
        "InputField"
        | "LabelField"
        | "CheckBoxField"
        | "PictureField"
        | "RadioButtonField"
        | "TextDocumentField"
        | "CalendarField"
        | "GraphicalSchemaField"
        | "SpreadSheetDocumentField"
        | "HTMLDocumentField" => [11usize, 12]
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
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    standard_command_owner_name_by_id: &BTreeMap<String, FormStandardCommandOwner>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let kind = fields.first()?.trim();
    if fields.len() == 1 && kind == "0" {
        return Some("0".to_string());
    }
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    if kind == "0" {
        if let Some(command_name) =
            form_standard_button_command_name(&uuid).or_else(|| form_standard_command_name(&uuid))
        {
            return Some(command_name.to_owned());
        }
        if let Some(command_name) = object_refs.get(&uuid) {
            return Some(form_object_reference_command_name(command_name));
        }
        return None;
    }
    if let Some(command_name) = commands
        .iter()
        .find(|command| command.id == kind && command.reference_uuid == uuid)
        .map(|command| format!("Form.Command.{}", command.name))
    {
        return Some(command_name);
    }
    if kind == "100"
        && let Some(reference) = object_refs.get(&uuid)
        && reference.starts_with("CommonForm.")
    {
        return Some(form_object_reference_command_name(reference));
    }
    if kind == "4"
        && let Some(reference) = object_refs.get(&uuid)
        && reference.starts_with("Catalog.")
    {
        return Some(format!("{reference}.StandardCommand.OpenByValue"));
    }
    let owner = standard_command_owner_name_by_id.get(kind)?;
    let standard = match owner.kind {
        FormStandardCommandOwnerKind::FormattedDocument => {
            form_formatted_document_standard_command_suffix(&uuid)
        }
        FormStandardCommandOwnerKind::GraphicalSchema => {
            form_graphical_schema_standard_command_suffix(&uuid)
        }
        FormStandardCommandOwnerKind::SpreadsheetDocument => {
            form_spreadsheet_document_standard_command_suffix(&uuid)
        }
        FormStandardCommandOwnerKind::Table => form_table_standard_command_suffix(&uuid),
    }?;
    Some(format!(
        "Form.Item.{}.StandardCommand.{standard}",
        owner.name
    ))
}

pub(super) fn form_standard_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        FORM_COMMAND_CUSTOMIZE_FORM_UUID => Some("Form.StandardCommand.CustomizeForm"),
        "fd8f031f-c168-4e1b-8b0c-15eb3057e688" => Some("Form.StandardCommand.Refresh"),
        "c32d43de-b820-49d0-bf7a-d70829f48f40" => Some("Form.StandardCommand.Delete"),
        "3dd3bd8a-ac1e-44d6-ac83-e7802642a5e2" => Some("Form.StandardCommand.Delete"),
        "1cc781aa-f32b-4dc7-996a-6c38c3deda5c" => Some("Form.StandardCommand.Delete"),
        "8d7bcd38-1bbb-4dc1-a9ad-cc9d5966ca8e" => Some("Form.StandardCommand.Start"),
        "e6a9041f-4d43-4f06-8e17-e95753531565" => Some("Form.StandardCommand.StartAndClose"),
        "389ef1f1-97ce-4326-adf5-886b2dead75c" => Some("Form.StandardCommand.UndoPosting"),
        "b520ca45-d8db-4982-b128-bb42a6afd911" => Some("Form.StandardCommand.FindByCurrentValue"),
        "c9abb6b0-eafd-4505-8312-9a7b6888cbf3" => Some("Form.StandardCommand.ChangeHistory"),
        "a2b927a1-35af-43e3-af73-4af22ac2c0fa" => Some("Form.StandardCommand.List"),
        "ffc5e8d5-40a7-4893-a590-49bd588f9466" => Some("Form.StandardCommand.HierarchicalList"),
        "0b83270d-7f95-4cdd-93c3-342d7991fed5" => Some("Form.StandardCommand.Tree"),
        "39c6a2fb-45cc-41b1-853f-967fb68aa1df" => Some("Form.StandardCommand.MoveItem"),
        "eb880cb2-a91f-4ad6-afb7-f0e6d7a1b111" => Some("Form.StandardCommand.SetDateInterval"),
        "62778a6d-6114-471c-93f7-e1ccd54bd266" => Some("Form.StandardCommand.CreateInitialImage"),
        "b08b7a35-583a-4756-b814-0436ff9139c0" => Some("Form.StandardCommand.LoadVariant"),
        "0fb774df-ec1c-4e23-9ed1-e089974f74bf" => Some("Form.StandardCommand.ReportSettings"),
        "8149a06a-dbf3-4d4d-a275-5385a4196fc7" => Some("Form.StandardCommand.CancelEdit"),
        "b0c9afb6-320c-4e36-be21-8f6d48116415" => Some("Form.StandardCommand.LoadReportSettings"),
        "03df6ee5-883c-4cc6-b319-d886d1a9b2c8" => Some("Form.StandardCommand.NewWindow"),
        "a11fe36e-0b45-4c07-80b3-2346b660a51e" => Some("Form.StandardCommand.Print"),
        "7910bb04-ddcc-4e5d-89f0-104c6ad0f187" => Some("Form.StandardCommand.SaveReportSettings"),
        "9bffcf73-7b1d-4a8d-bf23-5e051af3ee29" => Some("Form.StandardCommand.SaveVariant"),
        "5d41082e-9619-42ec-b96f-98b082b3a2f0" => Some("Form.StandardCommand.Yes"),
        "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d" => Some("Form.StandardCommand.No"),
        "68baa1bc-edd1-4d9b-ad80-1d53fb8a7988" => Some("Form.StandardCommand.Copy"),
        "342c531d-dc73-458a-8ac4-6a746916a33b" => Some("Form.StandardCommand.Copy"),
        "87317f86-057f-477e-9045-2da4e4980199" => Some("Form.StandardCommand.PostAndClose"),
        "96e0bc70-f8ff-4732-8119-060923203629" => Some("Form.StandardCommand.CancelSearch"),
        "9758d344-4b1d-4dc9-80bd-81060bc18b2a" => Some("Form.StandardCommand.OutputList"),
        "1c00edb8-a826-4855-9bde-94dbc5f620e5" => Some("Form.StandardCommand.ListSettings"),
        "1f317795-c420-4a30-b594-c492abc55f7a" => Some("Form.StandardCommand.Reread"),
        "3a17e914-ec6a-4280-b4df-78914f40522b" => Some("Form.StandardCommand.ShowInList"),
        "4f834c38-add1-45e4-a9f3-cefe3efac5c9" => Some("Form.StandardCommand.Create"),
        "3772996b-41f4-4c47-a5a8-ea397db424ae" => Some("Form.StandardCommand.Close"),
        "6886601d-276c-4d3f-af0a-05c586025608" => Some("Form.StandardCommand.Change"),
        "8e2b82cf-d1ea-46b2-afdf-a8d64e66ea2b" => Some("Form.StandardCommand.Choose"),
        "bdefa701-6685-453e-a02a-3683d0cc16d3" => Some("Form.StandardCommand.Find"),
        "3b8cedbc-8e74-4017-b901-d14b09f32f7a" => Some("Form.StandardCommand.Post"),
        "2e86453d-8958-4c9a-a1b4-b15215eedc2e" => Some("Form.StandardCommand.SetDeletionMark"),
        "827b541d-30c1-4f06-aecf-92aa496a0835" => Some("Form.StandardCommand.SetDeletionMark"),
        "39bb0fe9-771d-4dd5-8a6e-2d16984523af" => Some("Form.StandardCommand.Help"),
        "679b62d9-ff72-4329-bf3a-c0c32b311dd2" => Some("Form.StandardCommand.Cancel"),
        "32df4349-2607-4c2b-a4b9-bca4a1a28bd7" => Some("Form.StandardCommand.WriteAndClose"),
        "952c2984-9955-415a-8235-5c710aabe732" => {
            Some("Form.StandardCommand.LoadDynamicListSettings")
        }
        "d5c3842d-7252-4370-9174-756a6cc553e5" => {
            Some("Form.StandardCommand.SaveDynamicListSettings")
        }
        "d603a249-6eb3-4e38-bb2d-a8a86a8ab156" => {
            Some("Form.StandardCommand.DynamicListStandardSettings")
        }
        "d8772fd1-a3bf-417d-8334-c49968dbb45e" => Some("Form.StandardCommand.CreateFolder"),
        "f3613d5c-20c6-46e5-b4d5-7d712ece1296" => Some("Form.StandardCommand.OK"),
        "fe558fde-99b3-45d0-a060-9fc2905309f6" => Some("Form.StandardCommand.Write"),
        _ => None,
    }
}

pub(super) fn form_object_reference_command_name(reference: &str) -> String {
    if reference.contains(".Command.") || reference.starts_with("CommonCommand.") {
        return reference.to_string();
    }
    let Some((kind, _)) = reference.split_once('.') else {
        return reference.to_string();
    };
    let standard = super::command_interface::command_interface_standard_command(kind);
    match standard {
        Some(standard) => format!("{reference}.StandardCommand.{standard}"),
        None => reference.to_string(),
    }
}

pub(super) fn form_standard_button_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "239f0103-8de9-4fdf-b485-eb5531da7e51" => Some("Form.StandardCommand.SaveValues"),
        "71e0226e-ebb2-4e33-8745-0a94a01bbf15" => Some("Form.StandardCommand.RestoreValues"),
        _ => None,
    }
}

pub(super) fn form_formatted_document_standard_command_suffix(uuid: &str) -> Option<&'static str> {
    match uuid {
        "0bdc43a3-79f0-48d6-bce4-1142542e1a59" => Some("IncreaseFontSize"),
        "17724105-6e59-4d52-8a42-cf0fb4838037" => Some("BackColor"),
        "2b5d0007-b74c-4786-a904-37e64bda8414" => Some("NumberedList"),
        "b67f202a-dcf8-41f3-bda8-1ff9bed5f2ef" => Some("SelectAll"),
        "39f6b9f1-7aa1-4a03-a01b-e127d51bc228" => Some("DecreaseIndent"),
        "408f351e-0536-46be-8916-a891db9bfbe6" => Some("LineSpacing"),
        "4ca32834-6f9f-4dfb-89ce-6db36931c89b" => Some("Preview"),
        "56ae90b6-588f-406e-919c-cc5cc7f86297" => Some("AlignJustify"),
        "5a331cec-bf93-4af5-8f51-80fd7118db47" => Some("SaveAs"),
        "6d83186a-5838-40a5-95e7-8990193adf0a" => Some("Hyperlink"),
        "6f1ea963-0807-4de8-b544-b5666f500b05" => Some("Redo"),
        "71007f7d-1995-44aa-9125-9926e70a35b5" => Some("TextColor"),
        "7a294bdc-b86b-4b73-abc4-df9c811f61ef" => Some("CopyToClipboard"),
        "83670388-2e45-439e-9968-587eca6c7f8d" => Some("CutToClipboard"),
        "85bd789b-0047-46f9-9b2e-845907fc1b1d" => Some("Underline"),
        "871100d5-049d-4b22-a46a-fabf54bd64c3" => Some("Char"),
        "87ecfbdd-8e2b-4ba2-a315-0897020f382f" => Some("AlignLeft"),
        "905692d2-c3e7-4433-8f10-8d2ce35f652b" => Some("PasteFromClipboard"),
        "9d8a3915-de52-4227-91cd-2fce22e09972" => Some("Picture"),
        "a0033f06-56f7-4855-b901-7ac66fe1bb99" => Some("BulletedList"),
        "a8483976-8b13-416a-9680-133b306dc6b0" => Some("Print"),
        "a8631f01-318a-4da2-80a9-9075c7524463" => Some("Italic"),
        "a8f6b59e-b712-4d3e-a974-55a3be4eb295" => Some("Font"),
        "ab0ebc39-68ee-4034-b2f4-43eee55bd651" => Some("AlignCenter"),
        "d0a4d953-115b-4059-a6cb-6e67f903a4f3" => Some("IncreaseIndent"),
        "db1cd9b3-bdf4-43f5-abd6-c2e4bd85d709" => Some("Strikeout"),
        "e428af27-c4f7-4577-b80e-95a79f94322d" => Some("AlignRight"),
        "ec647dcc-2be7-486c-9046-d8b371f9909e" => Some("DecreaseFontSize"),
        "f20eefc2-f819-4ab1-be67-87b3ca2e26e6" => Some("Bold"),
        "f5814962-2bef-43dd-b633-a193d4b0970e" => Some("Undo"),
        _ => None,
    }
}

pub(super) fn form_graphical_schema_standard_command_suffix(uuid: &str) -> Option<&'static str> {
    match uuid {
        "e2d6f793-b786-4640-a91b-8d77f73860f1" => Some("Print"),
        "1d13f9a3-402a-46cb-9c68-1709356840f2" => Some("Preview"),
        "01db2225-b62d-4112-a4b6-d39d627bf79f" => Some("PageSetup"),
        _ => None,
    }
}

pub(super) fn form_spreadsheet_document_standard_command_suffix(
    uuid: &str,
) -> Option<&'static str> {
    match uuid {
        "02bdf755-7bc7-4e41-a199-f65cef10e6bc" => Some("SplitCell"),
        "05183b65-aa1f-401c-ac34-d99a6ff4ff60" => Some("BorderOutline"),
        "08872d94-c57a-470f-a050-0d4d81765df2" => Some("ThickBorderOutline"),
        "0a2d962b-5178-4fce-983b-19068b919f41" => Some("InsertColumnsRight"),
        "0cf34151-92d3-42fd-954f-5938433908a4" => Some("Find"),
        "0e355e57-d603-4ac6-998b-c522c43d3668" => Some("PrintImmediately"),
        "1177792e-7da2-4755-81a9-997f2d0e3dce" => Some("BorderNone"),
        "12acffde-8389-4e5e-bd86-ff248262d84a" => Some("ExpandAllGroups"),
        "17b9f6bb-74b3-439d-b719-eb236b2fe001" => Some("RowHeight"),
        "1ba33890-92e9-42a3-95bd-a5c783f46d55" => Some("CopyToClipboard"),
        "1b680da5-a5ca-4ea7-8db9-df079de39b61" => Some("Show"),
        "1d6dbce7-a813-437b-89b8-450319ed13bd" => Some("InsertRowsBottom"),
        "202efa3a-32e6-482c-8bc9-1a596bdd57f4" => Some("DeleteColumns"),
        "25d773e7-9961-49fc-a9c8-527079090143" => Some("GoToCell"),
        "29fc1bfc-7bf8-4849-9b4b-3e42d12ebcbb" => Some("FindNext"),
        "41c1bb40-1027-4fd1-a19e-17976100e64b" => Some("PageSetup"),
        "468dca2b-17be-4657-bae8-64b94fcf6187" => Some("InsertColumnsLeft"),
        "4a9ec98e-c814-47b6-911f-694effc10fc5" => Some("ShowHeaders"),
        "4ecc8cf1-2a26-446f-9fb6-93db4ffee068" => Some("InsertRowsTop"),
        "4efdcc95-9f24-4652-a5ed-febfaa51f135" => Some("BorderAll"),
        "531b9a9b-e4b3-4fe2-9562-1ff678c6d73d" => Some("DeleteComment"),
        "56ae90b6-588f-406e-919c-cc5cc7f86297" => Some("AlignJustify"),
        "59e67a77-8141-42cf-b062-7cb92e210b6d" => Some("ClearAll"),
        "5aa38159-2001-42ae-8451-f8cabe0762c3" => Some("Preview"),
        "5c52ec51-6000-4190-b8c8-bd6201a271f5" => Some("Edit"),
        "5ce7de77-4796-41a0-beb3-7dc420053639" => Some("SetName"),
        "5ff37d88-ba18-428c-bf49-9ccde4c46268" => Some("BorderBottom"),
        "635136cc-3a47-43d1-8893-d7d536bf37c4" => Some("BorderLeft"),
        "65ee8b9b-8a0a-4de8-9a71-3a7a8c43c4ba" => Some("ThickBorderBottom"),
        "7b63d2c2-5b6b-472e-8f6c-7438d952be73" => Some("DeleteRows"),
        "edf14e37-e755-4d1c-970c-48ed776e3a0e" => Some("PasteFromClipboard"),
        "80455469-5f1c-4817-a992-756dfee9138f" => Some("Text"),
        "81be7ad1-a93f-4936-92d0-d59c10213f28" => Some("BorderTop"),
        "852f0fba-4338-4c43-a2da-851fcffd07bb" => Some("Rectangle"),
        "85bd789b-0047-46f9-9b2e-845907fc1b1d" => Some("Underline"),
        "87ecfbdd-8e2b-4ba2-a315-0897020f382f" => Some("AlignLeft"),
        "88a56d46-abff-4925-91a2-6592a4664912" => Some("Ungroup"),
        "93d90e38-02a4-42f8-828a-2798f51c4500" => Some("Ellipse"),
        "97407339-2c9f-400b-bd5b-3d97b6d00c21" => Some("ColumnWidth"),
        "999b786e-0534-4fca-8b62-f706d5336798" => Some("SaveAs"),
        "a01654df-d7f1-4ec5-8b03-258d953de2e7" => Some("ThickBorderTop"),
        "a8631f01-318a-4da2-80a9-9075c7524463" => Some("Italic"),
        "a97ea34e-7af2-412c-aa9d-b3393b1914ac" => Some("Picture"),
        "ab0ebc39-68ee-4034-b2f4-43eee55bd651" => Some("AlignCenter"),
        "adc3d2d0-4d84-4038-a453-ab5d693a60bd" => Some("RemoveName"),
        "b34f83ab-1cd7-41b1-89bd-dd0804a47b26" => Some("FixTable"),
        "b55ad06a-ee91-4435-a747-6f51884772d9" => Some("ShowGroups"),
        "b573b54a-ce87-4078-bd21-4f06709157c6" => Some("Hide"),
        "be8800c3-8ccf-444a-bbf0-8f3078ff0ded" => Some("Properties"),
        "c4713141-07d1-411f-8804-172d4b8c4f01" => Some("BorderInside"),
        "c9b9e671-7c9b-44b5-97e9-dd1ee51a1bfd" => Some("Line"),
        "cca9b248-5f35-477f-a49d-95da3b7becad" => Some("SelectAll"),
        "d673d512-f71a-48a6-ae5d-527a64ffd813" => Some("Print"),
        "d8e20c4d-3519-49aa-80e5-d6d66fee741a" => Some("Save"),
        "e05dddd1-40ad-44a2-8ae7-0a551f0b1809" => Some("BorderRight"),
        "e406e2a0-f06b-4402-b8c3-9017c95df44c" => Some("Group"),
        "e428af27-c4f7-4577-b80e-95a79f94322d" => Some("AlignRight"),
        "e5c3a5a6-695d-41bf-9c88-4367fd2a2a6e" => Some("InsertRows"),
        "ed6630f2-c296-43dd-b408-d370513fcebc" => Some("InsertComment"),
        "f20eefc2-f819-4ab1-be67-87b3ca2e26e6" => Some("Bold"),
        "f4df676f-eefb-48bd-9117-83ee6c207cb5" => Some("Merge"),
        "ff533ae0-46a9-4e1d-aa3a-6dffa27e076b" => Some("SearchEverywhere"),
        "7eae9c22-db31-4f27-a56a-b4dd62d21a2c" => Some("ClearContent"),
        "ff5c34f8-b172-4ef2-91d3-48283a66a725" => Some("CollapseAllGroups"),
        "05e95a55-947e-4dde-a657-16ec11750a2f" => Some("Font"),
        "08fdfb5b-192a-41a9-b57a-9781cd3ef7b6" => Some("ShowRowAndColumnNames"),
        "0c66c888-7512-402c-941d-96bec0e5749a" => Some("ShowCellNames"),
        "0e8c7cb4-f146-4208-af36-b3f8c7d71b66" => Some("RemoveRepeatOnEachPage"),
        "14bd1c58-da9d-41db-a515-75f8b39fdc52" => Some("SendDrawingToBack"),
        "17724105-6e59-4d52-8a42-cf0fb4838037" => Some("BackColor"),
        "1c7e6bb5-54ac-4ebf-8823-e92b3cf629da" => Some("PageViewMode"),
        "2da58c85-ae4d-403f-b0e2-c50027a5467f" => Some("HeaderFooter"),
        "3a7ef674-f589-4734-9b22-954ea64dc79f" => Some("RemovePageBreak"),
        "3e15759b-551a-46c4-8d24-8d6df22a1a64" => Some("NextComment"),
        "41f3fbde-476a-4984-bd12-b32e990af811" => Some("RemovePrintArea"),
        "4402cb7a-f68e-44cd-9478-52a695b18a25" => Some("CombineToGroup"),
        "474180f3-c5bd-49bf-bfd4-fddb03918295" => Some("FindPrevious"),
        "49a22a23-d2cf-4f84-97ae-66f94f863145" => Some("BringDrawingForward"),
        "5ccf1fce-3fab-4fb6-ac04-a9b2cf689cee" => Some("EqualDrawingWidth"),
        "60abcc40-dc62-4d03-833b-7b8ab8232d2c" => Some("DistributeDrawingsHorizontally"),
        "6728e5c7-8f67-4b0d-bd6f-90b728218fe3" => Some("SetPrintArea"),
        "69333d9f-28d1-446b-bd9a-cf8f85cf1704" => Some("RemoveFromGroup"),
        "6f1ea963-0807-4de8-b544-b5666f500b05" => Some("Redo"),
        "71007f7d-1995-44aa-9125-9926e70a35b5" => Some("TextColor"),
        "719daaab-c2d0-473d-b373-faf18ebe7d9d" => Some("AlignDrawingMiddle"),
        "7e79f8d3-6cab-49d5-aac0-43f5056ed958" => Some("SendDrawingBackward"),
        "7f3f496d-506c-4239-98fb-58e1ea6ba54a" => Some("DistributeDrawingsVertically"),
        "80a0b41c-24df-40e4-8269-683fb557214d" => Some("AlignDrawingRight"),
        "83c3121e-60cc-4b47-a296-0c1976f0d766" => Some("ShowGrid"),
        "952af05e-0771-4c26-adb6-a3418a262e4a" => Some("InsertPageBreak"),
        "95dbc17e-d11e-4008-b9a9-24d5f5b1d061" => Some("ShowComments"),
        "9e525e9b-99ed-4d89-9f02-2bf449ba65e6" => Some("BlackAndWhiteView"),
        "9f71febd-8c22-4471-8410-31f455bb3c57" => Some("AlignDrawingBottom"),
        "b383fa5a-2324-4e7e-a166-aabb5d64aea3" => Some("BringDrawingToFront"),
        "c1c95e8a-37b4-477c-9df1-7bc0fdbc1bd3" => Some("BorderColor"),
        "c50fd6b2-51a1-47e0-8cd3-84b16823287c" => Some("RepeatOnEachPage"),
        "e1ae173a-22c3-4909-a72c-5454b64c6446" => Some("PreviousComment"),
        "ee0aab77-fd5f-4594-9c5e-e989a953642f" => Some("AlignDrawingLeft"),
        "f2b6b156-d929-4be2-af5b-9c9b792524bb" => Some("AlignDrawingCenter"),
        "f5773ab5-4036-49ca-8286-7a4ea2c354d7" => Some("EqualDrawingHeight"),
        "f5814962-2bef-43dd-b633-a193d4b0970e" => Some("Undo"),
        "f9395bfa-9301-4cec-8c1e-e2b62fb3abd6" => Some("AlignDrawingTop"),
        "fd523437-4160-4a52-a70b-9166c7eebcf0" => Some("EqualDrawingSize"),
        "feb51db7-bc1f-4b9f-a6e6-db24d5f812ab" => Some("Names"),
        _ => None,
    }
}

pub(super) fn form_table_standard_command_suffix(uuid: &str) -> Option<&'static str> {
    match uuid {
        "01833a5a-6553-4c49-b445-095018107bb5" => Some("HierarchicalList"),
        "05468165-f954-45a5-84f2-6641c51f9f23" => Some("Tree"),
        "0d0249a4-2b2f-4fc0-a66f-b36f9494b3cc" => Some("List"),
        "0e9b637d-cf6e-4330-8a8f-cd44842e34bb" => Some("LevelUp"),
        "0ae4bea5-23be-42a7-b69e-97b11b29c453" => Some("Copy"),
        "0f8d6d98-2f8b-405a-b8b3-0538e9d95da5" => Some("Create"),
        "12acffde-8389-4e5e-bd86-ff248262d84a" => Some("ExpandAllGroups"),
        "14559f7c-853c-42a4-9ea1-01546107747b" => Some("ListSettings"),
        "18248aa8-e621-4e19-a611-54fb8923644c" => Some("CheckAll"),
        "182a793b-22a5-4625-b316-6a5be7f88078" => Some("LoadDynamicListSettings"),
        "1f1e900a-8488-4159-81be-9704eb96906d" => Some("UserSettingItemProperties"),
        "27bd521a-51c6-4fe7-846d-a98f988774b5" => Some("MoveItem"),
        "37740564-9e86-44a0-bea9-3f485a5a3f91" => Some("MoveUp"),
        "2bbe4e12-06d2-409b-a972-eea585125d83" => Some("SortListAsc"),
        "33b7b9cd-6979-4435-8c58-d9bc8250edec" => Some("DynamicListStandardSettings"),
        "403bc6e6-b98e-4181-9f43-9c75cbbf82cf" => Some("Refresh"),
        "4a817da0-5797-4e16-906f-02fb869e1873" => Some("GroupFilterItems"),
        "51c99108-107c-43e1-8918-e48835bf2495" => Some("SelectAll"),
        "c0519548-2a9a-44de-a25e-faf01e089d4d" => Some("Find"),
        "44ad3ec9-f3c2-4913-9224-5f9fb6418743" => Some("CancelSearch"),
        "49602716-fea6-497f-8047-726404038857" => Some("OutputList"),
        "5048cc44-702b-44e3-8445-9af75c02724d" => Some("UncheckAll"),
        "58b2a785-23f6-4b0e-a324-9a1323285595" => Some("SortListDesc"),
        "59b4387d-f5be-4658-901f-bd3068217469" => Some("Pickup"),
        "5aa38159-2001-42ae-8451-f8cabe0762c3" => Some("Preview"),
        "714d44cc-63da-4431-b33a-428e398d2a08" => Some("FindByCurrentValue"),
        "7b683784-b474-441a-ba63-3d757bd0ffd4" => Some("SearchEverywhere"),
        "825c1c15-ef8f-47ab-b002-e6b84b3e5b10" => Some("OutputList"),
        "82b88a24-2856-484a-afd9-55a15bdf9785" => Some("Ungroup"),
        "88078230-1f6b-415f-99e4-ad2ff73810cf" => Some("CopyToClipboard"),
        "8969c93a-23e5-4bef-941d-aaef315858d2" => Some("Choose"),
        "8d772f97-c0ef-47c0-9cb0-efea28c61341" => Some("Delete"),
        "95b4bc12-2ece-4d7a-b3e2-6f9293620a06" => Some("SaveDynamicListSettings"),
        "9ef79140-3de6-436a-8dda-610bb963f5db" => Some("EndEdit"),
        "a2f737a8-0114-4e86-a214-45e5c213fa65" => Some("SetDeletionMark"),
        "a5fdef31-bbf0-4a9d-98aa-fd5fd8f1344a" => Some("AddFilterItemGroup"),
        "b0016a68-ec64-4e6d-b905-c71fd62efc4c" => Some("Add"),
        "b41f5bbc-ba5d-4888-8cd1-db246a371418" => Some("Change"),
        "d673d512-f71a-48a6-ae5d-527a64ffd813" => Some("Print"),
        "d7e55d2e-bfea-4d80-b4ad-a1bb31ec2147" => Some("UseFieldAsValue"),
        "d82ca05c-2966-4d77-9a39-a1eea087bfa7" => Some("CreateFolder"),
        "d8e20c4d-3519-49aa-80e5-d6d66fee741a" => Some("Save"),
        "daa306cd-a78a-4e74-a14c-739daba624cb" => Some("SetDateInterval"),
        "dc118d99-b351-4e30-9310-e864f2e53ec0" => Some("LevelDown"),
        "e7216412-03ac-4a81-99c2-1d7c28e88e31" => Some("ShowMultipleSelection"),
        "fca750bc-4fb6-40e2-ae0f-e818939a32e7" => Some("AddFilterItem"),
        "fa51b106-eae6-44c7-8054-76cbb3100603" => Some("MoveDown"),
        "ff5c34f8-b172-4ef2-91d3-48283a66a725" => Some("CollapseAllGroups"),
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
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    type_link_data_path_by_table_column: &BTreeMap<(String, String), String>,
    data_path_by_binding_key: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.as_slice() {
        [kind, owner] if kind.trim() == "1" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            if owner.len() != 1 {
                return None;
            }
            attribute_names_by_id.get(owner.first()?.trim()).cloned()
        }
        [kind, owner, terminal] if kind.trim() == "2" => {
            let owner = split_1c_braced_fields(owner.trim(), 0)?;
            let terminal = split_1c_braced_fields(terminal.trim(), 0)?;
            if terminal.len() != 1 {
                return None;
            }
            if owner.len() == 1 {
                if !matches!(terminal.first()?.trim(), "-5" | "-8") {
                    return None;
                }
                let attribute_name = attribute_names_by_id.get(owner.first()?.trim())?;
                return Some(format!("{attribute_name}.Ref"));
            }
            if owner.len() != 2 || owner.get(1)?.trim() != FORM_ITEM_TYPE_UUID {
                return None;
            }
            if let Some(data_path) = type_link_data_path_by_table_column.get(&(
                owner.first()?.trim().to_string(),
                terminal.first()?.trim().to_string(),
            )) {
                return Some(data_path.clone());
            }
            resolve_form_item_current_data_path(
                owner.first()?.trim(),
                terminal.first()?.trim(),
                table_name_by_id,
                table_column_names_by_id,
                data_path_by_binding_key,
            )
        }
        _ => None,
    }
}

fn resolve_form_item_current_data_path(
    table_id: &str,
    binding_key: &str,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
) -> Option<String> {
    let table_name = table_name_by_id.get(table_id)?;
    let field_name = if binding_key == "8" {
        "Ref".to_string()
    } else if let Some(field_name) = data_path_by_binding_key
        .get(binding_key)
        .and_then(|data_path| data_path.strip_prefix(table_name))
        .and_then(|field_name| field_name.strip_prefix('.'))
        .filter(|field_name| !field_name.is_empty())
    {
        normalize_form_table_column_name(table_name, field_name)
    } else if let Some(field_name) = table_column_names_by_id
        .get(table_id)
        .and_then(|columns| columns.get(binding_key))
    {
        normalize_form_table_column_name(table_name, field_name)
    } else {
        return None;
    };
    Some(format!("Items.{table_name}.CurrentData.{field_name}"))
}

#[cfg(test)]
pub(super) fn extract_form_command_interface(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterface> {
    extract_form_command_interface_with_commands(trailing, &[], object_refs)
}

#[cfg(test)]
pub(super) fn extract_form_command_interface_with_commands(
    trailing: &[String],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterface> {
    extract_form_command_interface_with_context(
        trailing,
        commands,
        object_refs,
        &BTreeMap::new(),
        None,
        &[],
        &FormChildItemIndexes::default(),
    )
}

pub(super) fn extract_form_command_interface_with_context(
    trailing: &[String],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    information_register_field_refs: &InformationRegisterFieldReferenceIndex,
    form_owner_reference: Option<&str>,
    attributes: &[FormAttribute],
    child_item_indexes: &FormChildItemIndexes,
) -> Option<FormCommandInterface> {
    let attribute_names_by_id = attributes
        .iter()
        .map(|attribute| (attribute.id.clone(), attribute.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let context = FormCommandInterfaceParseContext {
        commands,
        object_refs,
        information_register_field_refs,
        form_owner_reference,
        attribute_names_by_id: &attribute_names_by_id,
        child_item_indexes,
    };
    let mut command_bar = Vec::new();
    let mut navigation_panel = Vec::new();
    for (trailing_slot, field) in trailing.iter().enumerate() {
        let Some(container) =
            parse_form_command_interface_container(trailing_slot, field, &context)
        else {
            continue;
        };
        command_bar.extend(container.command_bar);
        navigation_panel.extend(container.navigation_panel);
    }
    (!command_bar.is_empty() || !navigation_panel.is_empty()).then_some(FormCommandInterface {
        command_bar,
        navigation_panel,
    })
}

pub(super) struct FormCommandInterfaceParseContext<'a> {
    commands: &'a [FormCommand],
    object_refs: &'a BTreeMap<String, String>,
    information_register_field_refs: &'a InformationRegisterFieldReferenceIndex,
    form_owner_reference: Option<&'a str>,
    attribute_names_by_id: &'a BTreeMap<String, String>,
    child_item_indexes: &'a FormChildItemIndexes,
}

pub(super) fn parse_form_command_interface_container(
    trailing_slot: usize,
    field: &str,
    context: &FormCommandInterfaceParseContext<'_>,
) -> Option<FormCommandInterface> {
    let fields = split_1c_braced_fields(field, 0)?;
    let declared_item_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let typed_item_count = fields
        .iter()
        .skip(2)
        .filter(|field| form_command_interface_item_schema(field).is_some())
        .count();
    let schema = FormCommandInterfaceContainerSchema::from_raw_layout(
        trailing_slot,
        fields.first()?.trim(),
        fields.len(),
        declared_item_count,
        typed_item_count,
    )?;
    let mut command_bar = Vec::new();
    let mut navigation_panel = Vec::new();
    for field in fields.iter().skip(2) {
        if let Some(item) = parse_form_command_interface_item(field, context) {
            match schema.owner() {
                FormCommandInterfaceContainerOwner::CommandBar => command_bar.push(item),
                FormCommandInterfaceContainerOwner::NavigationPanel => navigation_panel.push(item),
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
    context: &FormCommandInterfaceParseContext<'_>,
) -> Option<FormCommandInterfaceItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    FormCommandInterfaceItemSchema::from_raw_layout(
        fields.first()?.trim(),
        fields.len(),
        fields.get(4)?.trim(),
        fields.get(7)?.trim(),
    )?;
    form_command_interface_visibility_schema(fields.get(8)?)?;
    let command = parse_form_command_interface_command(fields.get(2)?, context)?;
    let item_type = parse_form_command_interface_item_type(fields.get(4).copied())?;
    let attribute = parse_form_command_interface_attribute(
        fields.get(3)?,
        context.attribute_names_by_id,
        context.child_item_indexes,
    )?;
    let command_group = fields
        .get(5)
        .and_then(|field| parse_form_command_group_reference(field, context.object_refs));
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
        attribute,
        command_group,
        index,
        default_visible,
        visible: parse_form_command_interface_visibility(fields.get(8)?, context.object_refs)?,
    })
}

pub(super) fn parse_form_command_interface_attribute(
    field: &str,
    attribute_names_by_id: &BTreeMap<String, String>,
    child_item_indexes: &FormChildItemIndexes,
) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.len() == 1 && fields.first()?.trim() == "0" {
        return Some(None);
    }
    if fields.len() != 3 || fields.first()?.trim() != "2" {
        return None;
    }
    let attribute = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let binding = split_1c_braced_fields(fields.get(2)?.trim(), 0)?;
    if attribute.len() != 1 || binding.len() != 1 {
        return None;
    }
    if binding.first()?.trim() == "-8" {
        let attribute_name = attribute_names_by_id.get(attribute.first()?.trim())?;
        return Some(Some(format!("{attribute_name}.Ref")));
    }
    parse_form_bound_data_path(
        field,
        "",
        attribute_names_by_id,
        &child_item_indexes.table_name_by_id,
        &child_item_indexes.table_column_names_by_id,
        &child_item_indexes.bound_table_path_by_binding_key,
        &child_item_indexes.table_column_names_by_binding_key,
    )
    .map(Some)
}

pub(super) fn form_command_interface_item_schema(
    field: &str,
) -> Option<FormCommandInterfaceItemSchema> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let schema = FormCommandInterfaceItemSchema::from_raw_layout(
        fields.first()?.trim(),
        fields.len(),
        fields.get(4)?.trim(),
        fields.get(7)?.trim(),
    )?;
    form_command_interface_visibility_schema(fields.get(8)?)?;
    Some(schema)
}

pub(super) fn form_command_interface_visibility_schema(
    field: &str,
) -> Option<FormCommandInterfaceVisibilitySchema> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let scope = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    parse_form_typed_bool(scope.get(1)?)?;
    let role_count = scope.get(2)?.trim().parse::<usize>().ok()?;
    let typed_role_count = scope
        .get(3..)?
        .chunks_exact(2)
        .filter(|pair| {
            parse_non_zero_uuid(pair[0].trim()).is_some()
                && parse_form_typed_bool(pair[1]).is_some()
        })
        .count();
    FormCommandInterfaceVisibilitySchema::from_raw_layout(
        fields.first()?.trim(),
        fields.len(),
        scope.first()?.trim(),
        scope.len(),
        role_count,
        typed_role_count,
    )
}

pub(super) fn parse_form_command_interface_visibility(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<FormCommandInterfaceVisibility>> {
    let schema = form_command_interface_visibility_schema(field)?;
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let scope = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let common = parse_form_typed_bool(scope.get(1)?)?;
    let mut role_values = Vec::with_capacity(schema.role_count());
    for pair in scope.get(3..)?.chunks_exact(2) {
        let uuid = parse_non_zero_uuid(pair[0].trim())?;
        let role = object_refs.get(&uuid)?.strip_prefix("Role.")?;
        role_values.push((format!("Role.{role}"), parse_form_typed_bool(pair[1])?));
    }
    Some(
        (!common || !role_values.is_empty()).then_some(FormCommandInterfaceVisibility {
            common,
            role_values,
        }),
    )
}

pub(super) fn parse_form_typed_bool(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.len() != 2 || fields.first().map(|value| value.trim()) != Some(r#""B""#) {
        return None;
    }
    match fields.get(1)?.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
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
    context: &FormCommandInterfaceParseContext<'_>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let kind = fields.first()?.trim();
    let target = fields.get(1).map(|value| value.trim());
    if kind != "0"
        && let Some(uuid) = target.and_then(parse_non_zero_uuid)
        && let Some(command) = context
            .commands
            .iter()
            .find(|command| command.id == kind && command.reference_uuid == uuid)
    {
        return Some(format!("Form.Command.{}", command.name));
    }
    match kind {
        "0" => {
            let Some(target) = target else {
                return Some("0".to_string());
            };
            if target == "0" || target == "00000000-0000-0000-0000-000000000000" {
                Some("0".to_string())
            } else {
                parse_non_zero_uuid(target).and_then(|uuid| context.object_refs.get(&uuid).cloned())
            }
        }
        "2" => {
            let uuid = parse_non_zero_uuid(target?)?;
            context
                .object_refs
                .get(&uuid)
                .map(|reference| format!("{reference}.StandardCommand.CreateBasedOn"))
        }
        "3" => {
            let uuid = parse_non_zero_uuid(target?)?;
            let reference = context.object_refs.get(&uuid)?;
            form_information_register_open_by_value_reference(reference)
                .or_else(|| {
                    let form_owner_reference = context.form_owner_reference?;
                    let field_reference = resolve_information_register_field_reference(
                        context.information_register_field_refs,
                        &uuid,
                        form_owner_reference,
                    )?;
                    field_reference
                        .strip_prefix(reference)
                        .filter(|suffix| suffix.starts_with('.'))?;
                    form_information_register_open_by_value_reference(field_reference)
                })
                .or_else(|| {
                    (reference.starts_with("CommonCommand.") || reference.contains(".Command."))
                        .then(|| reference.clone())
                })
        }
        "4" => {
            let uuid = parse_non_zero_uuid(target?)?;
            let reference = context.object_refs.get(&uuid)?;
            (reference.starts_with("Catalog.") && reference.matches('.').count() == 1)
                .then(|| format!("{reference}.StandardCommand.OpenByValue"))
        }
        _ => None,
    }
}

pub(super) fn resolve_information_register_field_reference<'a>(
    index: &'a InformationRegisterFieldReferenceIndex,
    register_uuid: &str,
    form_owner_reference: &str,
) -> Option<&'a str> {
    let mut matches = index
        .get(register_uuid)?
        .iter()
        .filter(|field| field.value_owner_references.contains(form_owner_reference));
    let field = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(&field.field_reference)
}

pub(super) fn form_information_register_open_by_value_reference(reference: &str) -> Option<String> {
    let parts = reference.split('.').collect::<Vec<_>>();
    if parts.len() != 4
        || parts.first().copied() != Some("InformationRegister")
        || !matches!(
            parts.get(2).copied(),
            Some("Dimension" | "Resource" | "Attribute")
        )
    {
        return None;
    }
    Some(format!(
        "{}.{}.StandardCommand.OpenByValue.{}",
        parts[0], parts[1], parts[3]
    ))
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
        "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0" => Some("FormCommandBarCreateBasedOn"),
        "cb50f5c0-8013-4262-93a2-f0db379d6b6b" => Some("FormCommandBarImportant"),
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1" => Some("FormNavigationPanelGoTo"),
        "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8" => Some("FormNavigationPanelSeeAlso"),
        "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2" => Some("FormNavigationPanelImportant"),
        _ => None,
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

fn append_form_document_properties_xml(xml: &mut String, properties: &FormBodyProperties) {
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
}

fn format_form_mobile_device_command_bar_content_xml(items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut xml = "\t<MobileDeviceCommandBarContent>\r\n".to_string();
    for item in items {
        xml.push_str("\t\t<xr:Item>\r\n");
        for property in FORM_MOBILE_DEVICE_COMMAND_BAR_CONTENT_ITEM_XML_ORDER {
            match property {
                FormMobileDeviceCommandBarContentItemXmlProperty::Presentation => {
                    xml.push_str("\t\t\t<xr:Presentation/>\r\n");
                }
                FormMobileDeviceCommandBarContentItemXmlProperty::CheckState => {
                    xml.push_str("\t\t\t<xr:CheckState>0</xr:CheckState>\r\n");
                }
                FormMobileDeviceCommandBarContentItemXmlProperty::Value if item.is_empty() => {
                    xml.push_str("\t\t\t<xr:Value xsi:type=\"xs:string\"/>\r\n");
                }
                FormMobileDeviceCommandBarContentItemXmlProperty::Value => {
                    xml.push_str(&format!(
                        "\t\t\t<xr:Value xsi:type=\"xs:string\">{}</xr:Value>\r\n",
                        escape_xml_text(item)
                    ));
                }
            }
        }
        xml.push_str("\t\t</xr:Item>\r\n");
    }
    xml.push_str("\t</MobileDeviceCommandBarContent>\r\n");
    xml
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
    if let Some(value) = properties.auto_save_data_in_settings {
        xml.push_str(&format!(
            "\t<AutoSaveDataInSettings>{}</AutoSaveDataInSettings>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.save_window_settings == Some(false) {
        xml.push_str("\t<SaveWindowSettings>false</SaveWindowSettings>\r\n");
    }
    if let Some(value) = properties.save_data_in_settings {
        xml.push_str(&format!(
            "\t<SaveDataInSettings>{}</SaveDataInSettings>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.auto_title == Some(false) {
        xml.push_str("\t<AutoTitle>false</AutoTitle>\r\n");
    }
    if properties.auto_url == Some(false) {
        xml.push_str("\t<AutoURL>false</AutoURL>\r\n");
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
    if properties.vertical_scroll.is_none() {
        append_form_document_properties_xml(&mut xml, properties);
    }
    if properties.auto_fill_check == Some(false) {
        xml.push_str("\t<AutoFillCheck>false</AutoFillCheck>\r\n");
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
    if let Some(vertical_scroll) = properties.vertical_scroll {
        xml.push_str(&format!(
            "\t<VerticalScroll>{}</VerticalScroll>\r\n",
            escape_xml_text(vertical_scroll)
        ));
    }
    if properties.vertical_scroll.is_some() {
        append_form_document_properties_xml(&mut xml, properties);
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
    xml.push_str(&format_form_mobile_device_command_bar_content_xml(
        &properties.mobile_device_command_bar_content,
    ));
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
    if let Some(value) = properties.use_for_folders_and_items {
        xml.push_str(&format!(
            "\t<UseForFoldersAndItems>{}</UseForFoldersAndItems>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.show_title == Some(false) {
        xml.push_str("\t<ShowTitle>false</ShowTitle>\r\n");
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
        let display_importance = command_bar
            .display_importance
            .map(|value| format!(" DisplayImportance=\"{}\"", escape_xml_text(value)))
            .unwrap_or_default();
        if command_bar.display_importance.is_some()
            || command_bar.horizontal_align.is_some()
            || command_bar.autofill == Some(false)
            || !command_bar.child_items.is_empty()
        {
            xml.push_str(&format!(
                "\t<AutoCommandBar name=\"{}\" id=\"{}\"{display_importance}>\r\n",
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
            if let Some(current_row_properties) = command.current_row_use.as_ref() {
                if let Some(current_row_use) = current_row_properties.value {
                    xml.push_str(&format!(
                        "\t\t\t<CurrentRowUse>{}</CurrentRowUse>\r\n",
                        escape_xml_text(current_row_use.xml_value())
                    ));
                }
                if let Some(associated_table_element_id) = current_row_properties
                    .associated_table_element_id
                    .as_deref()
                {
                    xml.push_str(&format!(
                        "\t\t\t<AssociatedTableElementId xsi:type=\"xs:string\">{}</AssociatedTableElementId>\r\n",
                        escape_xml_text(associated_table_element_id)
                    ));
                }
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

fn format_form_input_field_button_options_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "InputField" {
        return String::new();
    }
    let mut xml = String::new();
    for property in FORM_INPUT_FIELD_BUTTON_XML_ORDER {
        xml.push_str(&format_form_input_field_button_option_xml(
            item, *property, indent,
        ));
    }
    xml
}

fn format_form_input_field_button_option_xml(
    item: &FormChildItem,
    property: FormInputFieldXmlProperty,
    indent: usize,
) -> String {
    let tab = "\t".repeat(indent);
    match property {
        FormInputFieldXmlProperty::DropListButton => item
            .drop_list_button
            .map(|value| {
                format!(
                    "{tab}<DropListButton>{}</DropListButton>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormInputFieldXmlProperty::ChoiceButton => item
            .choice_button
            .map(|value| format!("{tab}<ChoiceButton>{}</ChoiceButton>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormInputFieldXmlProperty::ChoiceButtonRepresentation => item
            .choice_button_representation
            .map(|value| {
                format!(
                    "{tab}<ChoiceButtonRepresentation>{}</ChoiceButtonRepresentation>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormInputFieldXmlProperty::ClearButton => item
            .clear_button
            .map(|value| format!("{tab}<ClearButton>{}</ClearButton>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormInputFieldXmlProperty::SpinButton => item
            .spin_button
            .map(|value| format!("{tab}<SpinButton>{}</SpinButton>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormInputFieldXmlProperty::OpenButton => item
            .open_button
            .map(|value| format!("{tab}<OpenButton>{}</OpenButton>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormInputFieldXmlProperty::CreateButton => item
            .create_button
            .map(|value| format!("{tab}<CreateButton>{}</CreateButton>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormInputFieldXmlProperty::ChoiceListButton => item
            .choice_list_button
            .map(|value| {
                format!(
                    "{tab}<ChoiceListButton>{}</ChoiceListButton>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
    }
}

fn format_form_input_field_tail_xml(
    item: &FormChildItem,
    indent: usize,
    include_auto_mark_incomplete: bool,
) -> String {
    if item.tag != "InputField" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_INPUT_FIELD_TAIL_XML_ORDER {
        match property {
            FormInputFieldTailXmlProperty::ListChoiceMode
                if item.list_choice_mode == Some(true) =>
            {
                xml.push_str(&format!("{tab}<ListChoiceMode>true</ListChoiceMode>\r\n"));
            }
            FormInputFieldTailXmlProperty::ExtendedEditMultipleValues
                if item.extended_edit_multiple_values == Some(true) =>
            {
                xml.push_str(&format!(
                    "{tab}<ExtendedEditMultipleValues>true</ExtendedEditMultipleValues>\r\n"
                ));
            }
            FormInputFieldTailXmlProperty::AutoMarkIncomplete
                if include_auto_mark_incomplete
                    && item.extended_edit_multiple_values == Some(true)
                    && item.auto_mark_incomplete.is_some() =>
            {
                let value = item.auto_mark_incomplete == Some(true);
                xml.push_str(&format!(
                    "{tab}<AutoMarkIncomplete>{}</AutoMarkIncomplete>\r\n",
                    xml_bool(value)
                ));
            }
            _ => {}
        }
    }
    xml
}

fn format_form_auto_choice_incomplete_xml(item: &FormChildItem, indent: usize) -> String {
    if item.auto_choice_incomplete != Some(true) {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    format!("{tab}<AutoChoiceIncomplete>true</AutoChoiceIncomplete>\r\n")
}

fn format_form_direct_auto_mark_incomplete_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag == "Table"
        || (item.tag == "InputField" && item.extended_edit_multiple_values == Some(true))
    {
        return String::new();
    }
    format_form_auto_mark_incomplete_xml(item, indent)
}

fn format_form_auto_mark_incomplete_xml(item: &FormChildItem, indent: usize) -> String {
    let Some(value) = item.auto_mark_incomplete else {
        return String::new();
    };
    let tab = "\t".repeat(indent);
    format!(
        "{tab}<AutoMarkIncomplete>{}</AutoMarkIncomplete>\r\n",
        xml_bool(value)
    )
}

fn format_form_spreadsheet_document_properties_xml(item: &FormChildItem, indent: usize) -> String {
    let Some(properties) = item.spreadsheet_document_properties.as_ref() else {
        return String::new();
    };
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    if properties.show_grid == Some(true) {
        xml.push_str(&format!("{tab}<ShowGrid>true</ShowGrid>\r\n"));
    }
    if properties.show_headers == Some(true) {
        xml.push_str(&format!("{tab}<ShowHeaders>true</ShowHeaders>\r\n"));
    }
    if properties.show_cell_names == Some(true) {
        xml.push_str(&format!("{tab}<ShowCellNames>true</ShowCellNames>\r\n"));
    }
    if properties.show_row_and_column_names == Some(true) {
        xml.push_str(&format!(
            "{tab}<ShowRowAndColumnNames>true</ShowRowAndColumnNames>\r\n"
        ));
    }
    if let Some(value) = properties.vertical_scroll_bar {
        xml.push_str(&format!(
            "{tab}<VerticalScrollBar>{}</VerticalScrollBar>\r\n",
            xml_bool(value)
        ));
    }
    if let Some(value) = properties.horizontal_scroll_bar {
        xml.push_str(&format!(
            "{tab}<HorizontalScrollBar>{}</HorizontalScrollBar>\r\n",
            xml_bool(value)
        ));
    }
    if properties.edit == Some(true) {
        xml.push_str(&format!("{tab}<Edit>true</Edit>\r\n"));
    }
    if let Some(value) = properties.selection_show_mode {
        xml.push_str(&format!(
            "{tab}<SelectionShowMode>{}</SelectionShowMode>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.output {
        xml.push_str(&format!(
            "{tab}<Output>{}</Output>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.protection == Some(true) {
        xml.push_str(&format!("{tab}<Protection>true</Protection>\r\n"));
    }
    if properties.enable_start_drag == Some(false) {
        xml.push_str(&format!(
            "{tab}<EnableStartDrag>false</EnableStartDrag>\r\n"
        ));
    }
    if properties.enable_drag == Some(false) {
        xml.push_str(&format!("{tab}<EnableDrag>false</EnableDrag>\r\n"));
    }
    xml
}

fn format_form_table_properties_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "Table" {
        return String::new();
    }
    let mut xml = String::new();
    for property in FORM_TABLE_XML_ORDER {
        xml.push_str(&format_form_table_property_xml(item, *property, indent));
    }
    xml
}

fn format_form_table_property_xml(
    item: &FormChildItem,
    property: FormTableXmlProperty,
    indent: usize,
) -> String {
    let tab = "\t".repeat(indent);
    let hierarchical_table = form_table_has_hierarchical_navigation(item);
    match property {
        FormTableXmlProperty::Representation => item
            .table_representation
            .map(|value| {
                format!(
                    "{tab}<Representation>{}</Representation>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::TitleLocation => item
            .title_location
            .map(|value| {
                format!(
                    "{tab}<TitleLocation>{}</TitleLocation>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::UserVisible => match item.user_visible_common {
            Some(false) => format!(
                "{tab}<UserVisible>\r\n{tab}\t<xr:Common>false</xr:Common>\r\n{tab}</UserVisible>\r\n"
            ),
            _ => String::new(),
        },
        FormTableXmlProperty::Visible => match item.visible {
            Some(false) => format!("{tab}<Visible>false</Visible>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::CommandBarLocation => item
            .table_command_bar_location
            .map(|value| {
                format!(
                    "{tab}<CommandBarLocation>{}</CommandBarLocation>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::Autofill => match item.autofill {
            Some(true) => format!("{tab}<Autofill>true</Autofill>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::DefaultItem => match item.default_item {
            Some(true) => format!("{tab}<DefaultItem>true</DefaultItem>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::UseAlternationRowColor => item
            .use_alternation_row_color
            .map(|value| {
                format!(
                    "{tab}<UseAlternationRowColor>{}</UseAlternationRowColor>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::InitialTreeView => item
            .initial_tree_view
            .map(|value| {
                format!(
                    "{tab}<InitialTreeView>{}</InitialTreeView>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::AutoMarkIncomplete => match item.auto_mark_incomplete {
            Some(true) => format!("{tab}<AutoMarkIncomplete>true</AutoMarkIncomplete>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::SkipOnInput => item
            .skip_on_input
            .filter(|value| *value || should_emit_explicit_table_skip_on_input(item))
            .map(|value| format!("{tab}<SkipOnInput>{}</SkipOnInput>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::ReadOnly => match item.read_only {
            Some(true) => format!("{tab}<ReadOnly>true</ReadOnly>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::ChangeRowSet => match item.change_row_set {
            Some(false) => format!("{tab}<ChangeRowSet>false</ChangeRowSet>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::Width => item
            .width
            .as_ref()
            .map(|value| format!("{tab}<Width>{}</Width>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::Height => item
            .height
            .as_ref()
            .map(|value| format!("{tab}<Height>{}</Height>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::AutoMaxHeight => match item.auto_max_height {
            Some(false) => format!("{tab}<AutoMaxHeight>false</AutoMaxHeight>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::HeightInTableRows => item
            .height_in_table_rows
            .as_ref()
            .map(|value| {
                format!(
                    "{tab}<HeightInTableRows>{}</HeightInTableRows>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::ChangeRowOrder => match item.change_row_order {
            Some(false) => format!("{tab}<ChangeRowOrder>false</ChangeRowOrder>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::AutoMaxWidth => match item.auto_max_width {
            Some(false) if !hierarchical_table => {
                format!("{tab}<AutoMaxWidth>false</AutoMaxWidth>\r\n")
            }
            _ => String::new(),
        },
        FormTableXmlProperty::ChoiceMode => match item.table_choice_mode {
            Some(true) => format!("{tab}<ChoiceMode>true</ChoiceMode>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::RowInputMode => item
            .row_input_mode
            .map(|value| {
                format!(
                    "{tab}<RowInputMode>{}</RowInputMode>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::SelectionMode => item
            .table_selection_mode
            .map(|value| {
                format!(
                    "{tab}<SelectionMode>{}</SelectionMode>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::RowSelectionMode => match item.row_selection_mode {
            Some("Row") => format!("{tab}<RowSelectionMode>Row</RowSelectionMode>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::Header => match item.table_header {
            Some(false) => format!("{tab}<Header>false</Header>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::HorizontalLines => match item.table_horizontal_lines {
            Some(false) => format!("{tab}<HorizontalLines>false</HorizontalLines>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::VerticalLines => match item.table_vertical_lines {
            Some(false) => format!("{tab}<VerticalLines>false</VerticalLines>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::AutoInsertNewRow => match item.auto_insert_new_row {
            Some(true) => format!("{tab}<AutoInsertNewRow>true</AutoInsertNewRow>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::EnableStartDrag => match item.enable_start_drag {
            Some(true) => format!("{tab}<EnableStartDrag>true</EnableStartDrag>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::EnableDrag => match item.enable_drag {
            Some(true) => format!("{tab}<EnableDrag>true</EnableDrag>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::FileDragMode => item
            .file_drag_mode
            .map(|value| {
                format!(
                    "{tab}<FileDragMode>{}</FileDragMode>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::DataPath => item
            .data_path
            .as_ref()
            .map(|value| format!("{tab}<DataPath>{}</DataPath>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::RowPictureDataPath => item
            .row_picture_data_path
            .as_ref()
            .map(|value| {
                format!(
                    "{tab}<RowPictureDataPath>{}</RowPictureDataPath>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::RowsPicture => {
            if item.rows_picture_ref.is_none() && item.rows_picture_file_name.is_none() {
                String::new()
            } else {
                let mut xml = format!("{tab}<RowsPicture>\r\n");
                if let Some(reference) = &item.rows_picture_ref {
                    xml.push_str(&format!(
                        "{tab}\t<xr:Ref>{}</xr:Ref>\r\n",
                        escape_xml_text(reference)
                    ));
                } else if let Some(file_name) = &item.rows_picture_file_name {
                    xml.push_str(&format!(
                        "{tab}\t<xr:Abs>{}</xr:Abs>\r\n",
                        escape_xml_text(file_name)
                    ));
                }
                xml.push_str(&format!(
                    "{tab}\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}</RowsPicture>\r\n",
                    xml_bool(item.rows_picture_load_transparent)
                ));
                xml
            }
        }
        FormTableXmlProperty::BackColor => item
            .back_color
            .as_ref()
            .map(|value| format!("{tab}<BackColor>{}</BackColor>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::TextColor => item
            .text_color
            .as_ref()
            .map(|value| format!("{tab}<TextColor>{}</TextColor>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::BorderColor => item
            .border_color
            .as_ref()
            .map(|value| {
                format!(
                    "{tab}<BorderColor>{}</BorderColor>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::Title => format_form_localized_section("Title", &item.title, indent),
        FormTableXmlProperty::CommandSet => {
            if item.command_set_excluded_commands.is_empty() {
                String::new()
            } else {
                let mut xml = format!("{tab}<CommandSet>\r\n");
                for command in &item.command_set_excluded_commands {
                    xml.push_str(&format!(
                        "{tab}\t<ExcludedCommand>{}</ExcludedCommand>\r\n",
                        escape_xml_text(command)
                    ));
                }
                xml.push_str(&format!("{tab}</CommandSet>\r\n"));
                xml
            }
        }
        FormTableXmlProperty::ToolTip => {
            format_form_localized_section("ToolTip", &item.tooltip, indent)
        }
        FormTableXmlProperty::SearchStringLocation => item
            .table_search_string_location
            .map(|value| {
                format!(
                    "{tab}<SearchStringLocation>{}</SearchStringLocation>\r\n",
                    value.xml_value()
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::ViewStatusLocation => item
            .table_view_status_location
            .map(|value| {
                format!(
                    "{tab}<ViewStatusLocation>{}</ViewStatusLocation>\r\n",
                    value.xml_value()
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::SearchControlLocation => item
            .table_search_control_location
            .map(|value| {
                format!(
                    "{tab}<SearchControlLocation>{}</SearchControlLocation>\r\n",
                    value.xml_value()
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::AutoRefresh => item
            .auto_refresh
            .map(|value| format!("{tab}<AutoRefresh>{}</AutoRefresh>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::AutoRefreshPeriod => item
            .auto_refresh_period
            .as_ref()
            .map(|value| {
                format!(
                    "{tab}<AutoRefreshPeriod>{}</AutoRefreshPeriod>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::Period => item
            .period
            .as_ref()
            .map(|period| {
                format!(
                    "{tab}<Period>\r\n\
{tab}\t<v8:variant xsi:type=\"v8:StandardPeriodVariant\">{}</v8:variant>\r\n\
{tab}\t<v8:startDate>{}</v8:startDate>\r\n\
{tab}\t<v8:endDate>{}</v8:endDate>\r\n\
{tab}</Period>\r\n",
                    escape_xml_text(period.variant),
                    escape_xml_text(&period.start_date),
                    escape_xml_text(&period.end_date)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::ChoiceFoldersAndItems => item
            .choice_folders_and_items
            .map(|value| {
                format!(
                    "{tab}<ChoiceFoldersAndItems>{}</ChoiceFoldersAndItems>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::RestoreCurrentRow => item
            .restore_current_row
            .map(|value| {
                format!(
                    "{tab}<RestoreCurrentRow>{}</RestoreCurrentRow>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::TopLevelParent => match item.top_level_parent_nil {
            Some(true) => format!("{tab}<TopLevelParent xsi:nil=\"true\"/>\r\n"),
            _ => String::new(),
        },
        FormTableXmlProperty::ShowRoot => item
            .show_root
            .filter(|value| item.strict_table_schema || *value)
            .map(|value| format!("{tab}<ShowRoot>{}</ShowRoot>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormTableXmlProperty::AllowRootChoice => item
            .allow_root_choice
            .filter(|value| item.strict_table_schema || !*value)
            .map(|value| {
                format!(
                    "{tab}<AllowRootChoice>{}</AllowRootChoice>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::UpdateOnDataChange => item
            .update_on_data_change
            .map(|value| {
                format!(
                    "{tab}<UpdateOnDataChange>{}</UpdateOnDataChange>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::UserSettingsGroup => item
            .user_settings_group
            .as_ref()
            .map(|value| {
                format!(
                    "{tab}<UserSettingsGroup>{}</UserSettingsGroup>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormTableXmlProperty::AllowGettingCurrentRowURL => item
            .allow_getting_current_row_url
            .map(|value| {
                format!(
                    "{tab}<AllowGettingCurrentRowURL>{}</AllowGettingCurrentRowURL>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
    }
}

fn format_form_control_colors_xml(item: &FormChildItem, indent: usize) -> String {
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for (name, value) in [
        ("TextColor", item.text_color.as_ref()),
        ("BackColor", item.back_color.as_ref()),
        ("BorderColor", item.border_color.as_ref()),
    ] {
        if let Some(value) = value {
            xml.push_str(&format!(
                "{tab}<{name}>{}</{name}>\r\n",
                escape_xml_text(value)
            ));
        }
    }
    xml
}

fn format_form_command_bar_properties_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "CommandBar" {
        return String::new();
    }

    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    if let Some(width) = &item.width {
        xml.push_str(&format!(
            "{tab}<Width>{}</Width>\r\n",
            escape_xml_text(width)
        ));
    }
    if let Some(height) = &item.height {
        xml.push_str(&format!(
            "{tab}<Height>{}</Height>\r\n",
            escape_xml_text(height)
        ));
    }
    if let Some(horizontal_stretch) = item.horizontal_stretch {
        xml.push_str(&format!(
            "{tab}<HorizontalStretch>{}</HorizontalStretch>\r\n",
            xml_bool(horizontal_stretch)
        ));
    }
    if let Some(group_horizontal_align) = item.group_horizontal_align {
        xml.push_str(&format!(
            "{tab}<GroupHorizontalAlign>{}</GroupHorizontalAlign>\r\n",
            escape_xml_text(group_horizontal_align)
        ));
    }
    if let Some(group_vertical_align) = item.group_vertical_align {
        xml.push_str(&format!(
            "{tab}<GroupVerticalAlign>{}</GroupVerticalAlign>\r\n",
            escape_xml_text(group_vertical_align)
        ));
    }
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
    let display_importance = item
        .display_importance
        .map(|value| format!(" DisplayImportance=\"{}\"", escape_xml_text(value)))
        .unwrap_or_default();
    if item.tag == "AutoCommandBar" && item.auto_command_bar_empty_element {
        return format!(
            "{tab}<AutoCommandBar name=\"{}\" id=\"{}\"{display_importance}/>\r\n",
            escape_xml_text(&item.name),
            escape_xml_text(&item.id)
        );
    }
    let early_title_for_field = matches!(
        item.tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
            | "CalendarField"
            | "GraphicalSchemaField"
            | "SpreadSheetDocumentField"
            | "HTMLDocumentField"
            | "ProgressBarField"
            | "TrackBarField"
            | "ChartField"
            | "FormattedDocumentField"
            | "ColumnGroup"
    );
    let title_location_follows_title =
        FormFieldTitleLocationSchema::follows_title_in_xml(item.tag, !item.title.is_empty());
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
        "{tab}<{} name=\"{}\" id=\"{}\"{display_importance}>\r\n",
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
        && let Some(title_height) = &item.title_height
    {
        xml.push_str(&format!(
            "{tab}\t<TitleHeight>{}</TitleHeight>\r\n",
            escape_xml_text(title_height)
        ));
    }
    if item.tag == "Button" && item.visible == Some(false) {
        xml.push_str(&format!("{tab}\t<Visible>false</Visible>\r\n"));
    }
    if item.tag == "Button"
        && let Some(representation) = item.button_representation.filter(|value| *value != "None")
    {
        xml.push_str(&format!(
            "{tab}\t<Representation>{}</Representation>\r\n",
            escape_xml_text(representation)
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
    if item.tag == "AutoCommandBar" {
        if let Some(horizontal_align) = item
            .horizontal_align
            .and_then(FormChildItemAlignment::horizontal_align)
        {
            xml.push_str(&format!(
                "{tab}\t<HorizontalAlign>{}</HorizontalAlign>\r\n",
                escape_xml_text(horizontal_align)
            ));
        }
        if item.autofill == Some(false) {
            xml.push_str(&format!("{tab}\t<Autofill>false</Autofill>\r\n"));
        }
    }
    if item.tag == "Table" {
        xml.push_str(&format_form_table_properties_xml(item, indent + 1));
    }
    if item.tag != "Table"
        && item.tag != "Button"
        && let Some(data_path) = &item.data_path
    {
        xml.push_str(&format!(
            "{tab}\t<DataPath>{}</DataPath>\r\n",
            escape_xml_text(data_path)
        ));
    }
    if item.tag != "Table" && item.tag != "Button" && item.default_item == Some(true) {
        xml.push_str(&format!("{tab}\t<DefaultItem>true</DefaultItem>\r\n"));
    }
    if !matches!(item.tag, "Button" | "Table") && item.visible == Some(false) {
        xml.push_str(&format!("{tab}\t<Visible>false</Visible>\r\n"));
    }
    if item.tag != "Table" && item.user_visible_common == Some(false) {
        xml.push_str(&format!(
            "{tab}\t<UserVisible>\r\n{tab}\t\t<xr:Common>false</xr:Common>\r\n{tab}\t</UserVisible>\r\n"
        ));
    }
    if matches!(
        item.tag,
        "InputField" | "LabelField" | "CheckBoxField" | "PictureField" | "CommandBar"
    ) && item.enabled == Some(false)
    {
        xml.push_str(&format!("{tab}\t<Enabled>false</Enabled>\r\n"));
    }
    let read_only_before_title = item.tag != "Table"
        && item.read_only == Some(true)
        && matches!(
            item.tag,
            "InputField"
                | "LabelField"
                | "CheckBoxField"
                | "PictureField"
                | "TextDocumentField"
                | "GraphicalSchemaField"
                | "HTMLDocumentField"
                | "SpreadSheetDocumentField"
                | "FormattedDocumentField"
                | "ColumnGroup"
                | "Page"
        );
    if read_only_before_title {
        xml.push_str(&format!("{tab}\t<ReadOnly>true</ReadOnly>\r\n"));
    }
    if !matches!(
        item.tag,
        "Button" | "Table" | "LabelDecoration" | "PictureDecoration"
    ) && let Some(skip_on_input) = item.skip_on_input
    {
        xml.push_str(&format!(
            "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
            if skip_on_input { "true" } else { "false" }
        ));
    }
    if early_title_for_field {
        xml.push_str(&format_form_shared_container_content_change_xml(
            item,
            indent + 1,
        ));
        xml.push_str(&format_form_title_section(item, indent + 1));
        if title_location_follows_title && let Some(title_location) = item.title_location {
            xml.push_str(&format!(
                "{tab}\t<TitleLocation>{}</TitleLocation>\r\n",
                escape_xml_text(title_location)
            ));
        }
    }
    if item.tag != "UsualGroup"
        && let Some(title_font_xml) = &item.title_font_xml
    {
        xml.push_str(&format!("{tab}\t{title_font_xml}\r\n"));
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
    if !matches!(item.tag, "Table" | "UsualGroup")
        && item.read_only == Some(true)
        && !read_only_before_title
    {
        xml.push_str(&format!("{tab}\t<ReadOnly>true</ReadOnly>\r\n"));
    }
    if item.tag == "Button"
        && let Some(skip_on_input) = item.skip_on_input
    {
        xml.push_str(&format!(
            "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
            if skip_on_input { "true" } else { "false" }
        ));
    }
    if item.tag == "Button" && item.enabled == Some(false) {
        xml.push_str(&format!("{tab}\t<Enabled>false</Enabled>\r\n"));
    }
    if item.tag == "Button" && item.default_item == Some(true) {
        xml.push_str(&format!("{tab}\t<DefaultItem>true</DefaultItem>\r\n"));
    }
    if item.tag != "Table"
        && !title_location_follows_title
        && let Some(title_location) = item.title_location
    {
        xml.push_str(&format!(
            "{tab}\t<TitleLocation>{}</TitleLocation>\r\n",
            escape_xml_text(title_location)
        ));
    }
    if matches!(
        item.tag,
        "InputField" | "LabelField" | "CheckBoxField" | "PictureField" | "RadioButtonField"
    ) && let Some(title_height) = &item.title_height
    {
        xml.push_str(&format!(
            "{tab}\t<TitleHeight>{}</TitleHeight>\r\n",
            escape_xml_text(title_height)
        ));
    }
    if matches!(
        item.tag,
        "SpreadSheetDocumentField" | "FormattedDocumentField"
    ) && !item.command_set_excluded_commands.is_empty()
    {
        xml.push_str(&format!("{tab}\t<CommandSet>\r\n"));
        for command in &item.command_set_excluded_commands {
            xml.push_str(&format!(
                "{tab}\t\t<ExcludedCommand>{}</ExcludedCommand>\r\n",
                escape_xml_text(command)
            ));
        }
        xml.push_str(&format!("{tab}\t</CommandSet>\r\n"));
    }
    if matches!(
        item.tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "CalendarField"
            | "ProgressBarField"
            | "TrackBarField"
            | "ChartField"
            | "ColumnGroup"
    ) {
        xml.push_str(&format_form_localized_section(
            "ToolTip",
            &item.tooltip,
            indent + 1,
        ));
    }
    xml.push_str(&format_form_tooltip_representation_xml(
        item,
        FormTooltipRepresentationXmlOrder::FieldProperties,
        indent + 1,
    ));
    if item.tag != "Button"
        && item.tag != "PictureDecoration"
        && item.tag != "CommandBar"
        && let Some(group_vertical_align) = item.group_vertical_align
    {
        xml.push_str(&format!(
            "{tab}\t<GroupVerticalAlign>{}</GroupVerticalAlign>\r\n",
            escape_xml_text(group_vertical_align)
        ));
    }
    if !matches!(item.tag, "LabelDecoration" | "AutoCommandBar")
        && let Some(horizontal_align) = item
            .horizontal_align
            .and_then(FormChildItemAlignment::horizontal_align)
    {
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
    if item.cell_hyperlink == Some(true) {
        xml.push_str(&format!("{tab}\t<CellHyperlink>true</CellHyperlink>\r\n"));
    }
    if item.auto_cell_height == Some(true) {
        xml.push_str(&format!("{tab}\t<AutoCellHeight>true</AutoCellHeight>\r\n"));
    }
    if matches!(
        item.tag,
        "InputField" | "LabelField" | "CheckBoxField" | "PictureField"
    ) && item.show_in_header == Some(false)
    {
        xml.push_str(&format!("{tab}\t<ShowInHeader>false</ShowInHeader>\r\n"));
    }
    xml.push_str(&format_form_field_header_picture_xml(item, indent + 1));
    if item.show_in_footer == Some(false) {
        xml.push_str(&format!("{tab}\t<ShowInFooter>false</ShowInFooter>\r\n"));
    }
    if let Some(footer_horizontal_align) = item.footer_horizontal_align {
        xml.push_str(&format!(
            "{tab}\t<FooterHorizontalAlign>{}</FooterHorizontalAlign>\r\n",
            escape_xml_text(footer_horizontal_align)
        ));
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
    xml.push_str(&format_form_usual_group_properties_xml(
        item,
        FormUsualGroupXmlAnchor::BeforeTitle,
        indent + 1,
    ));
    xml.push_str(&format_form_usual_group_header_xml(item, indent + 1));
    xml.push_str(&format_form_label_decoration_geometry_xml(item, indent + 1));
    xml.push_str(&format_form_picture_decoration_geometry_xml(
        item,
        indent + 1,
    ));
    if item.tag != "Table"
        && item.tag != "Button"
        && item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && item.tag != "CommandBar"
        && let Some(width) = &item.width
    {
        xml.push_str(&format!(
            "{tab}\t<Width>{}</Width>\r\n",
            escape_xml_text(width)
        ));
    }
    if item.tag != "Table"
        && item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && item.auto_max_width == Some(false)
    {
        xml.push_str(&format!("{tab}\t<AutoMaxWidth>false</AutoMaxWidth>\r\n"));
    }
    if item.tag != "Table"
        && item.tag != "Button"
        && item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && item.tag != "CommandBar"
        && let Some(height) = &item.height
    {
        xml.push_str(&format!(
            "{tab}\t<Height>{}</Height>\r\n",
            escape_xml_text(height)
        ));
    }
    if item.tag == "CalendarField" {
        if let Some(show_current_date) = item.show_current_date {
            xml.push_str(&format!(
                "{tab}\t<ShowCurrentDate>{}</ShowCurrentDate>\r\n",
                xml_bool(show_current_date)
            ));
        }
        if let Some(show_months_panel) = item.show_months_panel {
            xml.push_str(&format!(
                "{tab}\t<ShowMonthsPanel>{}</ShowMonthsPanel>\r\n",
                xml_bool(show_months_panel)
            ));
        }
        if let Some(width_in_months) = &item.width_in_months {
            xml.push_str(&format!(
                "{tab}\t<WidthInMonths>{}</WidthInMonths>\r\n",
                escape_xml_text(width_in_months)
            ));
        }
        if let Some(height_in_months) = &item.height_in_months {
            xml.push_str(&format!(
                "{tab}\t<HeightInMonths>{}</HeightInMonths>\r\n",
                escape_xml_text(height_in_months)
            ));
        }
    }
    if item.tag == "LabelDecoration"
        && let Some(skip_on_input) = item.skip_on_input
    {
        xml.push_str(&format!(
            "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
            if skip_on_input { "true" } else { "false" }
        ));
    }
    if item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && let Some(max_width) = &item.max_width
    {
        xml.push_str(&format!(
            "{tab}\t<MaxWidth>{}</MaxWidth>\r\n",
            escape_xml_text(max_width)
        ));
    }
    if item.tag == "Button" {
        if let Some(height) = &item.height {
            xml.push_str(&format!(
                "{tab}\t<Height>{}</Height>\r\n",
                escape_xml_text(height)
            ));
        }
        if let Some(horizontal_stretch) = item.horizontal_stretch {
            xml.push_str(&format!(
                "{tab}\t<HorizontalStretch>{}</HorizontalStretch>\r\n",
                xml_bool(horizontal_stretch)
            ));
        }
        if let Some(vertical_stretch) = item.vertical_stretch {
            xml.push_str(&format!(
                "{tab}\t<VerticalStretch>{}</VerticalStretch>\r\n",
                xml_bool(vertical_stretch)
            ));
        }
        if let Some(group_horizontal_align) = item.group_horizontal_align {
            xml.push_str(&format!(
                "{tab}\t<GroupHorizontalAlign>{}</GroupHorizontalAlign>\r\n",
                escape_xml_text(group_horizontal_align)
            ));
        }
        if let Some(group_vertical_align) = item.group_vertical_align {
            xml.push_str(&format!(
                "{tab}\t<GroupVerticalAlign>{}</GroupVerticalAlign>\r\n",
                escape_xml_text(group_vertical_align)
            ));
        }
    }
    if item.tag == "Button"
        && let Some(command_name) = &item.command_name
    {
        xml.push_str(&format!(
            "{tab}\t<CommandName>{}</CommandName>\r\n",
            escape_xml_text(command_name)
        ));
    }
    if item.tag == "Button" {
        xml.push_str(&format_form_control_colors_xml(item, indent + 1));
    }
    if item.tag == "Button"
        && let Some(data_path) = &item.data_path
    {
        xml.push_str(&format!(
            "{tab}\t<DataPath>{}</DataPath>\r\n",
            escape_xml_text(data_path)
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
        && !matches!(item.tag, "Button" | "InputField" | "Table")
        && let Some(text_color) = &item.text_color
    {
        xml.push_str(&format!(
            "{tab}\t<TextColor>{}</TextColor>\r\n",
            escape_xml_text(text_color)
        ));
    }
    if item.tag != "Table"
        && item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && item.auto_max_height == Some(false)
    {
        xml.push_str(&format!("{tab}\t<AutoMaxHeight>false</AutoMaxHeight>\r\n"));
    }
    if item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && let Some(max_height) = &item.max_height
    {
        xml.push_str(&format!(
            "{tab}\t<MaxHeight>{}</MaxHeight>\r\n",
            escape_xml_text(max_height)
        ));
    }
    if item.tag != "Button"
        && item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && item.tag != "Page"
        && item.tag != "CommandBar"
        && let Some(horizontal_stretch) = item.horizontal_stretch
        && !usual_group_title_first
    {
        xml.push_str(&format!(
            "{tab}\t<HorizontalStretch>{}</HorizontalStretch>\r\n",
            if horizontal_stretch { "true" } else { "false" }
        ));
    }
    if !matches!(item.tag, "Table" | "InputField")
        && let Some(choice_folders_and_items) = item.choice_folders_and_items
    {
        xml.push_str(&format!(
            "{tab}\t<ChoiceFoldersAndItems>{}</ChoiceFoldersAndItems>\r\n",
            escape_xml_text(choice_folders_and_items)
        ));
    }
    if item.tag != "Button"
        && item.tag != "LabelDecoration"
        && item.tag != "PictureDecoration"
        && item.tag != "Page"
        && let Some(vertical_stretch) = item.vertical_stretch
        && !usual_group_title_first
    {
        xml.push_str(&format!(
            "{tab}\t<VerticalStretch>{}</VerticalStretch>\r\n",
            if vertical_stretch { "true" } else { "false" }
        ));
    }
    xml.push_str(&format_form_spreadsheet_document_properties_xml(
        item,
        indent + 1,
    ));
    if let Some(max_value) = &item.max_value {
        xml.push_str(&format!(
            "{tab}\t<MaxValue>{}</MaxValue>\r\n",
            escape_xml_text(max_value)
        ));
    }
    if item.show_percent == Some(true) {
        xml.push_str(&format!("{tab}\t<ShowPercent>true</ShowPercent>\r\n"));
    }
    if item.tag == "PictureField"
        && let Some(picture_size) = item
            .picture_size
            .filter(|value| should_emit_form_picture_size(value))
    {
        xml.push_str(&format!(
            "{tab}\t<PictureSize>{}</PictureSize>\r\n",
            escape_xml_text(picture_size)
        ));
    }
    if item.tag == "PictureField" {
        if let Some(reference) = &item.picture_ref {
            xml.push_str(&format!(
                "{tab}\t<ValuesPicture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</ValuesPicture>\r\n",
                escape_xml_text(reference),
                xml_bool(item.picture_load_transparent)
            ));
        }
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
    if let Some(extended_edit) = item.extended_edit {
        xml.push_str(&format!(
            "{tab}\t<ExtendedEdit>{}</ExtendedEdit>\r\n",
            if extended_edit { "true" } else { "false" }
        ));
    }
    if item.mask.is_none() {
        xml.push_str(&format_form_auto_choice_incomplete_xml(item, indent + 1));
        if item.tag != "InputField" {
            xml.push_str(&format_form_direct_auto_mark_incomplete_xml(
                item,
                indent + 1,
            ));
        }
    }
    xml.push_str(&format_form_input_field_button_options_xml(
        item,
        indent + 1,
    ));
    if let Some(mask) = &item.mask {
        xml.push_str(&format!(
            "{tab}\t<Mask>{}</Mask>\r\n",
            escape_xml_text(mask)
        ));
        xml.push_str(&format_form_input_field_tail_xml(item, indent + 1, false));
        xml.push_str(&format_form_auto_choice_incomplete_xml(item, indent + 1));
    } else {
        xml.push_str(&format_form_input_field_tail_xml(item, indent + 1, false));
    }
    if let Some(quick_choice) = item.quick_choice {
        xml.push_str(&format!(
            "{tab}\t<QuickChoice>{}</QuickChoice>\r\n",
            if quick_choice { "true" } else { "false" }
        ));
    }
    if item.tag == "InputField" {
        if let Some(choice_folders_and_items) = item.choice_folders_and_items {
            xml.push_str(&format!(
                "{tab}\t<ChoiceFoldersAndItems>{}</ChoiceFoldersAndItems>\r\n",
                escape_xml_text(choice_folders_and_items)
            ));
        }
        xml.push_str(&format_form_auto_mark_incomplete_xml(item, indent + 1));
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
    }
    if item.choose_type == Some(false) {
        xml.push_str(&format!("{tab}\t<ChooseType>false</ChooseType>\r\n"));
    }
    if let Some(incomplete_choice_mode) = item.incomplete_choice_mode {
        xml.push_str(&format!(
            "{tab}\t<IncompleteChoiceMode>{}</IncompleteChoiceMode>\r\n",
            escape_xml_text(incomplete_choice_mode)
        ));
    }
    if item.text_edit == Some(false) {
        xml.push_str(&format!("{tab}\t<TextEdit>false</TextEdit>\r\n"));
    }
    if let Some(edit_text_update) = item.edit_text_update {
        xml.push_str(&format!(
            "{tab}\t<EditTextUpdate>{}</EditTextUpdate>\r\n",
            escape_xml_text(edit_text_update)
        ));
    }
    if let Some(reference) = &item.choice_button_picture_ref {
        xml.push_str(&format!(
            "{tab}\t<ChoiceButtonPicture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</ChoiceButtonPicture>\r\n",
            escape_xml_text(reference),
            xml_bool(item.choice_button_picture_load_transparent)
        ));
    }
    if let Some(value) = &item.input_min_value {
        xml.push_str(&format!(
            "{tab}\t<MinValue xsi:type=\"xs:decimal\">{}</MinValue>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = &item.input_max_value {
        xml.push_str(&format!(
            "{tab}\t<MaxValue xsi:type=\"xs:decimal\">{}</MaxValue>\r\n",
            escape_xml_text(value)
        ));
    }
    if !item.choice_list.is_empty() {
        xml.push_str(&format_form_choice_list_xml(&item.choice_list, indent + 1));
    }
    if let Some(drop_list_width) = &item.drop_list_width {
        xml.push_str(&format!(
            "{tab}\t<DropListWidth>{}</DropListWidth>\r\n",
            escape_xml_text(drop_list_width)
        ));
    }
    if item.tag == "InputField" && !item.choice_parameter_links.is_empty() {
        xml.push_str(&format_form_choice_parameter_links_xml(
            &item.choice_parameter_links,
            indent + 1,
        ));
    }
    if item.tag == "InputField"
        && let Some(type_link) = &item.type_link
    {
        xml.push_str(&format_form_type_link_xml(type_link, indent + 1));
    }
    if item.tag == "InputField" && !item.input_hint.is_empty() {
        xml.push_str(&format_form_localized_section(
            "InputHint",
            &item.input_hint,
            indent + 1,
        ));
    }
    if usual_group_title_first {
        if item.tag == "ButtonGroup" {
            xml.push_str(&format_form_shared_container_content_change_xml(
                item,
                indent + 1,
            ));
            xml.push_str(&format_form_localized_section(
                "Title",
                &item.title,
                indent + 1,
            ));
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
            if let Some(vertical_stretch) = item.vertical_stretch {
                xml.push_str(&format!(
                    "{tab}\t<VerticalStretch>{}</VerticalStretch>\r\n",
                    if vertical_stretch { "true" } else { "false" }
                ));
            }
        } else if item.horizontal_stretch == Some(true) {
            xml.push_str(&format!(
                "{tab}\t<HorizontalStretch>true</HorizontalStretch>\r\n"
            ));
        }
    }
    xml.push_str(&format_form_usual_group_properties_xml(
        item,
        FormUsualGroupXmlAnchor::BeforeGroup,
        indent + 1,
    ));
    if item.tag != "Page"
        && let Some(group) = item.group
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
    xml.push_str(&format_form_usual_group_properties_xml(
        item,
        FormUsualGroupXmlAnchor::BeforeBehavior,
        indent + 1,
    ));
    if let Some(behavior) = item.behavior {
        xml.push_str(&format!(
            "{tab}\t<Behavior>{}</Behavior>\r\n",
            escape_xml_text(behavior)
        ));
    }
    xml.push_str(&format_form_usual_group_properties_xml(
        item,
        FormUsualGroupXmlAnchor::AfterBehavior,
        indent + 1,
    ));
    if !matches!(item.tag, "Pages" | "Popup")
        && let Some(representation) = item.representation.filter(|representation| {
            !form_child_item_representation_is_default(item.tag, representation)
        })
    {
        xml.push_str(&format!(
            "{tab}\t<Representation>{}</Representation>\r\n",
            escape_xml_text(representation)
        ));
    }
    xml.push_str(&format_form_usual_group_properties_xml(
        item,
        FormUsualGroupXmlAnchor::AfterRepresentation,
        indent + 1,
    ));
    if item.tag != "Page" && item.show_title == Some(false) {
        xml.push_str(&format!("{tab}\t<ShowTitle>false</ShowTitle>\r\n"));
    }
    xml.push_str(&format_form_usual_group_properties_xml(
        item,
        FormUsualGroupXmlAnchor::AfterShowTitle,
        indent + 1,
    ));
    if item.tag == "ColumnGroup" && item.show_in_header == Some(true) {
        xml.push_str(&format!("{tab}\t<ShowInHeader>true</ShowInHeader>\r\n"));
    }
    if !matches!(item.tag, "UsualGroup" | "InputField") && !item.format.is_empty() {
        xml.push_str(&format_form_localized_section(
            "Format",
            &item.format,
            indent + 1,
        ));
    }
    if item.tag != "InputField" && !item.edit_format.is_empty() {
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
    if item.tag == "PictureDecoration"
        && let Some(skip_on_input) = item.skip_on_input
    {
        xml.push_str(&format!(
            "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
            xml_bool(skip_on_input)
        ));
    }
    if item.tag == "Page" {
        xml.push_str(&format_form_page_properties_xml(item, indent + 1));
    } else if !early_title_for_field && !usual_group_title_first && item.tag != "Table" {
        xml.push_str(&format_form_shared_container_content_change_xml(
            item,
            indent + 1,
        ));
        if matches!(item.tag, "LabelDecoration" | "PictureDecoration") {
            xml.push_str(&format_form_decoration_header_xml(item, indent + 1));
        } else {
            xml.push_str(&format_form_title_section(item, indent + 1));
            if title_location_follows_title && let Some(title_location) = item.title_location {
                xml.push_str(&format!(
                    "{tab}\t<TitleLocation>{}</TitleLocation>\r\n",
                    escape_xml_text(title_location)
                ));
            }
        }
        if item.tag == "LabelDecoration" && item.hiperlink == Some(true) {
            xml.push_str(&format!("{tab}\t<Hyperlink>true</Hyperlink>\r\n"));
        }
        if item.tag == "LabelDecoration" {
            xml.push_str(&format_form_label_decoration_alignment_tail_xml(
                item,
                indent + 1,
            ));
            xml.push_str(&format_form_label_decoration_visual_tail_xml(
                item,
                indent + 1,
            ));
        }
        if item.tag == "PictureDecoration" {
            if let Some(picture_size) = item
                .picture_size
                .filter(|value| should_emit_form_picture_size(value))
            {
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
            if let Some(file_name) = item.picture_file_name {
                xml.push_str(&format!(
                    "{tab}\t<Picture>\r\n\
{tab}\t\t<xr:Abs>{}</xr:Abs>\r\n\
{tab}\t\t<xr:LoadTransparent>false</xr:LoadTransparent>\r\n\
{tab}\t</Picture>\r\n",
                    escape_xml_text(file_name)
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
    if item.tag == "CommandBar" {
        xml.push_str(&format_form_localized_section(
            "ToolTip",
            &item.tooltip,
            indent + 1,
        ));
    }
    xml.push_str(&format_form_command_bar_properties_xml(item, indent + 1));
    if item.tag == "Popup" {
        xml.push_str(&format_form_localized_section(
            "ToolTip",
            &item.tooltip,
            indent + 1,
        ));
    }
    xml.push_str(&format_form_tooltip_representation_xml(
        item,
        FormTooltipRepresentationXmlOrder::AfterTitle,
        indent + 1,
    ));
    if item.tag == "Button"
        && let Some(shape_representation) = item.shape_representation
    {
        xml.push_str(&format!(
            "{tab}\t<ShapeRepresentation>{}</ShapeRepresentation>\r\n",
            escape_xml_text(shape_representation)
        ));
    }
    if item.tag == "Button"
        && let Some(representation) = item.representation_in_context_menu
    {
        xml.push_str(&format!(
            "{tab}\t<RepresentationInContextMenu>{}</RepresentationInContextMenu>\r\n",
            escape_xml_text(representation)
        ));
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
    if item.tag == "ButtonGroup"
        && let Some(command_source) = &item.command_source
    {
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
    if item.tag == "Popup"
        && let Some(command_source) = &item.command_source
    {
        xml.push_str(&format!(
            "{tab}\t<CommandSource>{}</CommandSource>\r\n",
            escape_xml_text(command_source)
        ));
    }
    if item.tag == "Popup"
        && let Some(representation) = item.representation
    {
        xml.push_str(&format!(
            "{tab}\t<Representation>{}</Representation>\r\n",
            escape_xml_text(representation)
        ));
    }
    if !matches!(
        item.tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "CalendarField"
            | "ProgressBarField"
            | "TrackBarField"
            | "ChartField"
            | "ColumnGroup"
            | "Table"
            | "LabelDecoration"
            | "PictureDecoration"
            | "UsualGroup"
            | "ButtonGroup"
            | "Popup"
            | "Page"
            | "CommandBar"
    ) {
        xml.push_str(&format_form_localized_section(
            "ToolTip",
            &item.tooltip,
            indent + 1,
        ));
    }
    if item.tag == "CommandBar"
        && let Some(command_source) = &item.command_source
    {
        xml.push_str(&format!(
            "{tab}\t<CommandSource>{}</CommandSource>\r\n",
            escape_xml_text(command_source)
        ));
    }
    if item.tag == "InputField" {
        xml.push_str(&format_form_control_colors_xml(item, indent + 1));
    }
    if item.tag == "PictureField"
        && let Some(file_drag_mode) = item.file_drag_mode
    {
        xml.push_str(&format!(
            "{tab}\t<FileDragMode>{}</FileDragMode>\r\n",
            escape_xml_text(file_drag_mode)
        ));
    }
    if matches!(
        item.tag,
        "LabelField" | "LabelDecoration" | "HTMLDocumentField" | "SpreadSheetDocumentField"
    ) {
        if let Some(back_color) = &item.back_color {
            xml.push_str(&format!(
                "{tab}\t<BackColor>{}</BackColor>\r\n",
                escape_xml_text(back_color)
            ));
        }
        if let Some(border_color) = &item.border_color {
            xml.push_str(&format!(
                "{tab}\t<BorderColor>{}</BorderColor>\r\n",
                escape_xml_text(border_color)
            ));
        }
    }
    if let Some(choice_history_on_input) = item.choice_history_on_input {
        xml.push_str(&format!(
            "{tab}\t<ChoiceHistoryOnInput>{}</ChoiceHistoryOnInput>\r\n",
            escape_xml_text(choice_history_on_input)
        ));
    }
    if !direct_context_menu_xml.is_empty() {
        xml.push_str(&direct_context_menu_xml);
    }
    if item.tag == "Pages"
        && let Some(representation) = item.representation.filter(|representation| {
            !form_child_item_representation_is_default(item.tag, representation)
        })
    {
        xml.push_str(&format!(
            "{tab}\t<PagesRepresentation>{}</PagesRepresentation>\r\n",
            escape_xml_text(representation)
        ));
    }
    if matches!(item.tag, "Page" | "UsualGroup")
        && let Some(title_data_path) = &item.title_data_path
    {
        xml.push_str(&format!(
            "{tab}\t<TitleDataPath>{}</TitleDataPath>\r\n",
            escape_xml_text(title_data_path)
        ));
    }
    if item.tag != "Table"
        && let Some(extended_tooltip) = &item.extended_tooltip
    {
        xml.push_str(&format_form_extended_tooltip_xml(
            extended_tooltip,
            indent + 1,
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
        if let Some(extended_tooltip) = &item.extended_tooltip {
            xml.push_str(&format_form_extended_tooltip_xml(
                extended_tooltip,
                indent + 1,
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

fn format_form_page_properties_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "Page" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_PAGE_XML_ORDER {
        match property {
            FormPageXmlProperty::EnableContentChange => {
                if let Some(enable_content_change) = item.enable_content_change {
                    xml.push_str(&format!(
                        "{tab}<EnableContentChange>{}</EnableContentChange>\r\n",
                        xml_bool(enable_content_change)
                    ));
                }
            }
            FormPageXmlProperty::Title => {
                xml.push_str(&format_form_title_section(item, indent));
            }
            FormPageXmlProperty::ToolTip => {
                xml.push_str(&format_form_localized_section(
                    "ToolTip",
                    &item.tooltip,
                    indent,
                ));
            }
            FormPageXmlProperty::ToolTipRepresentation => {
                if let Some(tooltip_representation) = item.tooltip_representation {
                    xml.push_str(&format!(
                        "{tab}<ToolTipRepresentation>{}</ToolTipRepresentation>\r\n",
                        escape_xml_text(tooltip_representation)
                    ));
                }
            }
            FormPageXmlProperty::Picture => {
                if let Some(reference) = &item.picture_ref {
                    xml.push_str(&format!(
                        "{tab}<Picture>\r\n\
{tab}\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}</Picture>\r\n",
                        escape_xml_text(reference),
                        xml_bool(item.picture_load_transparent)
                    ));
                }
            }
            FormPageXmlProperty::HorizontalStretch => {
                if let Some(horizontal_stretch) = item.horizontal_stretch {
                    xml.push_str(&format!(
                        "{tab}<HorizontalStretch>{}</HorizontalStretch>\r\n",
                        xml_bool(horizontal_stretch)
                    ));
                }
            }
            FormPageXmlProperty::VerticalStretch => {
                if let Some(vertical_stretch) = item.vertical_stretch {
                    xml.push_str(&format!(
                        "{tab}<VerticalStretch>{}</VerticalStretch>\r\n",
                        xml_bool(vertical_stretch)
                    ));
                }
            }
            FormPageXmlProperty::Group => {
                if let Some(group) = item.group {
                    xml.push_str(&format!(
                        "{tab}<Group>{}</Group>\r\n",
                        escape_xml_text(group)
                    ));
                }
            }
            FormPageXmlProperty::HorizontalAlign => {
                if let Some(horizontal_align) = item.usual_group_horizontal_align {
                    xml.push_str(&format!(
                        "{tab}<HorizontalAlign>{}</HorizontalAlign>\r\n",
                        escape_xml_text(horizontal_align)
                    ));
                }
            }
            FormPageXmlProperty::VerticalAlign => {
                if let Some(vertical_align) = item.usual_group_vertical_align {
                    xml.push_str(&format!(
                        "{tab}<VerticalAlign>{}</VerticalAlign>\r\n",
                        escape_xml_text(vertical_align)
                    ));
                }
            }
            FormPageXmlProperty::ChildItemsWidth => {
                if let Some(child_items_width) = item.child_items_width {
                    xml.push_str(&format!(
                        "{tab}<ChildItemsWidth>{}</ChildItemsWidth>\r\n",
                        escape_xml_text(child_items_width)
                    ));
                }
            }
            FormPageXmlProperty::ShowTitle => {
                if item.show_title == Some(false) {
                    xml.push_str(&format!("{tab}<ShowTitle>false</ShowTitle>\r\n"));
                }
            }
            FormPageXmlProperty::BackColor => {
                if let Some(back_color) = &item.back_color {
                    xml.push_str(&format!(
                        "{tab}<BackColor>{}</BackColor>\r\n",
                        escape_xml_text(back_color)
                    ));
                }
            }
        }
    }
    xml
}

fn format_form_shared_container_content_change_xml(item: &FormChildItem, indent: usize) -> String {
    if !FormSharedContainerContentChangeSchema::supports_xml_tag(item.tag)
        || item.enable_content_change != Some(true)
    {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    format!("{tab}<EnableContentChange>true</EnableContentChange>\r\n")
}

fn format_form_decoration_header_xml(item: &FormChildItem, indent: usize) -> String {
    if !matches!(item.tag, "LabelDecoration" | "PictureDecoration") {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_DECORATION_HEADER_XML_ORDER {
        match property {
            FormDecorationHeaderXmlProperty::Title => {
                xml.push_str(&format_form_title_section(item, indent));
            }
            FormDecorationHeaderXmlProperty::ToolTip => {
                xml.push_str(&format_form_localized_section(
                    "ToolTip",
                    &item.tooltip,
                    indent,
                ));
            }
            FormDecorationHeaderXmlProperty::ToolTipRepresentation => {
                xml.push_str(&format_form_tooltip_representation_xml(
                    item,
                    FormTooltipRepresentationXmlOrder::DecorationHeader,
                    indent,
                ));
            }
            FormDecorationHeaderXmlProperty::GroupHorizontalAlign => {
                if let Some(group_horizontal_align) = item.group_horizontal_align {
                    xml.push_str(&format!(
                        "{tab}<GroupHorizontalAlign>{}</GroupHorizontalAlign>\r\n",
                        escape_xml_text(group_horizontal_align)
                    ));
                }
            }
            FormDecorationHeaderXmlProperty::GroupVerticalAlign => {
                let group_vertical_align = if item.tag == "PictureDecoration" {
                    item.group_vertical_align
                } else {
                    item.horizontal_align
                        .and_then(FormChildItemAlignment::group_vertical_align)
                };
                if let Some(group_vertical_align) = group_vertical_align {
                    xml.push_str(&format!(
                        "{tab}<GroupVerticalAlign>{}</GroupVerticalAlign>\r\n",
                        escape_xml_text(group_vertical_align)
                    ));
                }
            }
        }
    }
    xml
}

fn format_form_label_decoration_geometry_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "LabelDecoration" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_LABEL_DECORATION_GEOMETRY_XML_ORDER {
        match property {
            FormLabelDecorationGeometryXmlProperty::Width => {
                if let Some(width) = &item.width {
                    xml.push_str(&format!(
                        "{tab}<Width>{}</Width>\r\n",
                        escape_xml_text(width)
                    ));
                }
            }
            FormLabelDecorationGeometryXmlProperty::AutoMaxWidth => {
                if item.auto_max_width == Some(false) {
                    xml.push_str(&format!("{tab}<AutoMaxWidth>false</AutoMaxWidth>\r\n"));
                }
            }
            FormLabelDecorationGeometryXmlProperty::MaxWidth => {
                if let Some(max_width) = &item.max_width {
                    xml.push_str(&format!(
                        "{tab}<MaxWidth>{}</MaxWidth>\r\n",
                        escape_xml_text(max_width)
                    ));
                }
            }
            FormLabelDecorationGeometryXmlProperty::Height => {
                if let Some(height) = &item.height {
                    xml.push_str(&format!(
                        "{tab}<Height>{}</Height>\r\n",
                        escape_xml_text(height)
                    ));
                }
            }
            FormLabelDecorationGeometryXmlProperty::AutoMaxHeight => {
                if item.auto_max_height == Some(false) {
                    xml.push_str(&format!("{tab}<AutoMaxHeight>false</AutoMaxHeight>\r\n"));
                }
            }
            FormLabelDecorationGeometryXmlProperty::MaxHeight => {
                if let Some(max_height) = &item.max_height {
                    xml.push_str(&format!(
                        "{tab}<MaxHeight>{}</MaxHeight>\r\n",
                        escape_xml_text(max_height)
                    ));
                }
            }
            FormLabelDecorationGeometryXmlProperty::HorizontalStretch => {
                if let Some(horizontal_stretch) = item.horizontal_stretch {
                    xml.push_str(&format!(
                        "{tab}<HorizontalStretch>{}</HorizontalStretch>\r\n",
                        if horizontal_stretch { "true" } else { "false" }
                    ));
                }
            }
            FormLabelDecorationGeometryXmlProperty::VerticalStretch => {
                if let Some(vertical_stretch) = item.vertical_stretch {
                    xml.push_str(&format!(
                        "{tab}<VerticalStretch>{}</VerticalStretch>\r\n",
                        if vertical_stretch { "true" } else { "false" }
                    ));
                }
            }
        }
    }
    xml
}

fn format_form_picture_decoration_geometry_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "PictureDecoration" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_PICTURE_DECORATION_GEOMETRY_XML_ORDER {
        match property {
            FormPictureDecorationGeometryXmlProperty::Width => {
                if let Some(width) = &item.width {
                    xml.push_str(&format!(
                        "{tab}<Width>{}</Width>\r\n",
                        escape_xml_text(width)
                    ));
                }
            }
            FormPictureDecorationGeometryXmlProperty::AutoMaxWidth => {
                if item.auto_max_width == Some(false) {
                    xml.push_str(&format!("{tab}<AutoMaxWidth>false</AutoMaxWidth>\r\n"));
                }
            }
            FormPictureDecorationGeometryXmlProperty::MaxWidth => {
                if let Some(max_width) = &item.max_width {
                    xml.push_str(&format!(
                        "{tab}<MaxWidth>{}</MaxWidth>\r\n",
                        escape_xml_text(max_width)
                    ));
                }
            }
            FormPictureDecorationGeometryXmlProperty::Height => {
                if let Some(height) = &item.height {
                    xml.push_str(&format!(
                        "{tab}<Height>{}</Height>\r\n",
                        escape_xml_text(height)
                    ));
                }
            }
            FormPictureDecorationGeometryXmlProperty::AutoMaxHeight => {
                if item.auto_max_height == Some(false) {
                    xml.push_str(&format!("{tab}<AutoMaxHeight>false</AutoMaxHeight>\r\n"));
                }
            }
            FormPictureDecorationGeometryXmlProperty::MaxHeight => {
                if let Some(max_height) = &item.max_height {
                    xml.push_str(&format!(
                        "{tab}<MaxHeight>{}</MaxHeight>\r\n",
                        escape_xml_text(max_height)
                    ));
                }
            }
            FormPictureDecorationGeometryXmlProperty::HorizontalStretch => {
                if let Some(horizontal_stretch) = item.horizontal_stretch {
                    xml.push_str(&format!(
                        "{tab}<HorizontalStretch>{}</HorizontalStretch>\r\n",
                        xml_bool(horizontal_stretch)
                    ));
                }
            }
            FormPictureDecorationGeometryXmlProperty::VerticalStretch => {
                if let Some(vertical_stretch) = item.vertical_stretch {
                    xml.push_str(&format!(
                        "{tab}<VerticalStretch>{}</VerticalStretch>\r\n",
                        xml_bool(vertical_stretch)
                    ));
                }
            }
        }
    }
    xml
}

fn format_form_label_decoration_alignment_tail_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "LabelDecoration" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_LABEL_DECORATION_ALIGNMENT_TAIL_XML_ORDER {
        let value = match property {
            FormLabelDecorationAlignmentTailXmlProperty::HorizontalAlign => item
                .horizontal_align
                .and_then(FormChildItemAlignment::horizontal_align),
            FormLabelDecorationAlignmentTailXmlProperty::VerticalAlign => item
                .horizontal_align
                .and_then(FormChildItemAlignment::vertical_align),
        };
        if let Some(value) = value {
            let tag_name = match property {
                FormLabelDecorationAlignmentTailXmlProperty::HorizontalAlign => "HorizontalAlign",
                FormLabelDecorationAlignmentTailXmlProperty::VerticalAlign => "VerticalAlign",
            };
            xml.push_str(&format!(
                "{tab}<{tag_name}>{}</{tag_name}>\r\n",
                escape_xml_text(value)
            ));
        }
    }
    xml
}

fn format_form_label_decoration_visual_tail_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "LabelDecoration" {
        return String::new();
    }
    let Some(visual_tail) = &item.label_decoration_visual_tail else {
        return String::new();
    };
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_LABEL_DECORATION_VISUAL_TAIL_XML_ORDER {
        match property {
            FormLabelDecorationVisualTailXmlProperty::TitleHeight => {
                if let Some(title_height) = visual_tail.title_height() {
                    xml.push_str(&format!(
                        "{tab}<TitleHeight>{}</TitleHeight>\r\n",
                        escape_xml_text(title_height)
                    ));
                }
            }
        }
    }
    xml
}

fn format_form_usual_group_properties_xml(
    item: &FormChildItem,
    anchor: FormUsualGroupXmlAnchor,
    indent: usize,
) -> String {
    if item.tag != "UsualGroup" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_USUAL_GROUP_XML_ORDER {
        if property.anchor() != anchor {
            continue;
        }
        match property {
            FormUsualGroupXmlProperty::ReadOnly => {
                if item.read_only == Some(true) {
                    xml.push_str(&format!("{tab}<ReadOnly>true</ReadOnly>\r\n"));
                }
            }
            FormUsualGroupXmlProperty::Enabled => {
                if item.usual_group_enabled == Some(false) {
                    xml.push_str(&format!("{tab}<Enabled>false</Enabled>\r\n"));
                }
            }
            FormUsualGroupXmlProperty::EnableContentChange => {
                if item.enable_content_change == Some(true) {
                    xml.push_str(&format!(
                        "{tab}<EnableContentChange>true</EnableContentChange>\r\n"
                    ));
                }
            }
            FormUsualGroupXmlProperty::GroupHorizontalAlign => {
                if let Some(value) = item.group_horizontal_align {
                    xml.push_str(&format!(
                        "{tab}<GroupHorizontalAlign>{}</GroupHorizontalAlign>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::GroupVerticalAlign => {
                if let Some(value) = item.usual_group_group_vertical_align {
                    xml.push_str(&format!(
                        "{tab}<GroupVerticalAlign>{}</GroupVerticalAlign>\r\n",
                        value.xml_value()
                    ));
                }
            }
            FormUsualGroupXmlProperty::ChildrenAlign => {
                if let Some(value) = item.usual_group_children_align {
                    xml.push_str(&format!(
                        "{tab}<ChildrenAlign>{}</ChildrenAlign>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::HorizontalSpacing => {
                if let Some(value) = item.usual_group_horizontal_spacing {
                    xml.push_str(&format!(
                        "{tab}<HorizontalSpacing>{}</HorizontalSpacing>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::VerticalSpacing => {
                if let Some(value) = item.usual_group_vertical_spacing {
                    xml.push_str(&format!(
                        "{tab}<VerticalSpacing>{}</VerticalSpacing>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::HorizontalAlign => {
                if let Some(value) = item.usual_group_horizontal_align {
                    xml.push_str(&format!(
                        "{tab}<HorizontalAlign>{}</HorizontalAlign>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::VerticalAlign => {
                if let Some(value) = item.usual_group_vertical_align {
                    xml.push_str(&format!(
                        "{tab}<VerticalAlign>{}</VerticalAlign>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::CollapsedRepresentationTitle => {
                xml.push_str(&format_form_localized_section(
                    "CollapsedRepresentationTitle",
                    &item.usual_group_collapsed_representation_title,
                    indent,
                ));
            }
            FormUsualGroupXmlProperty::Collapsed => {
                if item.collapsed == Some(true) {
                    xml.push_str(&format!("{tab}<Collapsed>true</Collapsed>\r\n"));
                }
            }
            FormUsualGroupXmlProperty::ControlRepresentation => {
                if let Some(value) = item.control_representation {
                    xml.push_str(&format!(
                        "{tab}<ControlRepresentation>{}</ControlRepresentation>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::Format => {
                xml.push_str(&format_form_localized_section(
                    "Format",
                    &item.format,
                    indent,
                ));
            }
            FormUsualGroupXmlProperty::ShowLeftMargin => {
                if item.usual_group_show_left_margin == Some(false) {
                    xml.push_str(&format!("{tab}<ShowLeftMargin>false</ShowLeftMargin>\r\n"));
                }
            }
            FormUsualGroupXmlProperty::United => {
                if item.united == Some(false) {
                    xml.push_str(&format!("{tab}<United>false</United>\r\n"));
                }
            }
            FormUsualGroupXmlProperty::ChildItemsWidth => {
                if let Some(value) = item.child_items_width {
                    xml.push_str(&format!(
                        "{tab}<ChildItemsWidth>{}</ChildItemsWidth>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::BackColor => {
                if let Some(value) = &item.back_color {
                    xml.push_str(&format!(
                        "{tab}<BackColor>{}</BackColor>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
            FormUsualGroupXmlProperty::ThroughAlign => {
                if let Some(value) = item.through_align {
                    xml.push_str(&format!(
                        "{tab}<ThroughAlign>{}</ThroughAlign>\r\n",
                        escape_xml_text(value)
                    ));
                }
            }
        }
    }
    xml
}

fn format_form_usual_group_header_xml(item: &FormChildItem, indent: usize) -> String {
    if item.tag != "UsualGroup" {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = String::new();
    for property in FORM_USUAL_GROUP_HEADER_XML_ORDER {
        match property {
            FormUsualGroupHeaderXmlProperty::Title => {
                xml.push_str(&format_form_localized_section("Title", &item.title, indent));
            }
            FormUsualGroupHeaderXmlProperty::Shortcut => {
                if let Some(shortcut) = &item.usual_group_shortcut {
                    xml.push_str(&format!(
                        "{tab}<Shortcut>{}</Shortcut>\r\n",
                        escape_xml_text(shortcut)
                    ));
                }
            }
            FormUsualGroupHeaderXmlProperty::TitleTextColor => {
                if let Some(title_text_color) = &item.title_text_color {
                    xml.push_str(&format!(
                        "{tab}<TitleTextColor>{}</TitleTextColor>\r\n",
                        escape_xml_text(title_text_color)
                    ));
                }
            }
            FormUsualGroupHeaderXmlProperty::TitleFont => {
                if let Some(title_font_xml) = &item.title_font_xml {
                    xml.push_str(&format!("{tab}{title_font_xml}\r\n"));
                }
            }
            FormUsualGroupHeaderXmlProperty::ToolTip => {
                xml.push_str(&format_form_localized_section(
                    "ToolTip",
                    &item.tooltip,
                    indent,
                ));
            }
            FormUsualGroupHeaderXmlProperty::ToolTipRepresentation => {
                xml.push_str(&format_form_tooltip_representation_xml(
                    item,
                    FormTooltipRepresentationXmlOrder::UsualGroupHeader,
                    indent,
                ));
            }
        }
    }
    xml
}

fn format_form_tooltip_representation_xml(
    item: &FormChildItem,
    xml_order: FormTooltipRepresentationXmlOrder,
    indent: usize,
) -> String {
    if form_tooltip_representation_xml_order(item.tag) != Some(xml_order) {
        return String::new();
    }
    item.tooltip_representation
        .map(|value| {
            format!(
                "{}<ToolTipRepresentation>{}</ToolTipRepresentation>\r\n",
                "\t".repeat(indent),
                escape_xml_text(value)
            )
        })
        .unwrap_or_default()
}

fn format_form_field_header_picture_xml(item: &FormChildItem, indent: usize) -> String {
    if item.header_picture_ref.is_none() && item.header_picture_file_name.is_none() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<HeaderPicture>\r\n");
    for property in FORM_FIELD_HEADER_PICTURE_XML_ORDER {
        match property {
            FormFieldHeaderPictureXmlProperty::Value => {
                if let Some(reference) = &item.header_picture_ref {
                    xml.push_str(&format!(
                        "{tab}\t<xr:Ref>{}</xr:Ref>\r\n",
                        escape_xml_text(reference)
                    ));
                } else if let Some(file_name) = &item.header_picture_file_name {
                    xml.push_str(&format!(
                        "{tab}\t<xr:Abs>{}</xr:Abs>\r\n",
                        escape_xml_text(file_name)
                    ));
                }
            }
            FormFieldHeaderPictureXmlProperty::LoadTransparent => {
                xml.push_str(&format!(
                    "{tab}\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n",
                    xml_bool(item.header_picture_load_transparent)
                ));
            }
        }
    }
    xml.push_str(&format!("{tab}</HeaderPicture>\r\n"));
    xml
}

fn should_emit_form_picture_size(picture_size: &str) -> bool {
    picture_size != "RealSize"
}

fn format_form_extended_tooltip_xml(tooltip: &FormExtendedTooltip, indent: usize) -> String {
    let tab = "\t".repeat(indent);
    if !tooltip.has_properties() {
        return format!(
            "{tab}<ExtendedTooltip name=\"{}\" id=\"{}\"/>\r\n",
            escape_xml_text(&tooltip.name),
            escape_xml_text(&tooltip.id)
        );
    }

    let mut xml = format!(
        "{tab}<ExtendedTooltip name=\"{}\" id=\"{}\">\r\n",
        escape_xml_text(&tooltip.name),
        escape_xml_text(&tooltip.id)
    );
    for property in FORM_EXTENDED_TOOLTIP_XML_ORDER {
        xml.push_str(&format_form_extended_tooltip_property_xml(
            tooltip,
            *property,
            indent + 1,
        ));
    }
    xml.push_str(&format!("{tab}</ExtendedTooltip>\r\n"));
    xml
}

fn format_form_extended_tooltip_property_xml(
    tooltip: &FormExtendedTooltip,
    property: FormExtendedTooltipXmlProperty,
    indent: usize,
) -> String {
    let tab = "\t".repeat(indent);
    match property {
        FormExtendedTooltipXmlProperty::Width => tooltip
            .width
            .as_ref()
            .map(|value| format!("{tab}<Width>{}</Width>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::AutoMaxWidth => tooltip
            .auto_max_width
            .map(|value| format!("{tab}<AutoMaxWidth>{}</AutoMaxWidth>\r\n", xml_bool(value)))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::MaxWidth => tooltip
            .max_width
            .as_ref()
            .map(|value| format!("{tab}<MaxWidth>{}</MaxWidth>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::Height => tooltip
            .height
            .as_ref()
            .map(|value| format!("{tab}<Height>{}</Height>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::AutoMaxHeight => tooltip
            .auto_max_height
            .map(|value| {
                format!(
                    "{tab}<AutoMaxHeight>{}</AutoMaxHeight>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::HorizontalStretch => tooltip
            .horizontal_stretch
            .map(|value| {
                format!(
                    "{tab}<HorizontalStretch>{}</HorizontalStretch>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::VerticalStretch => tooltip
            .vertical_stretch
            .map(|value| {
                format!(
                    "{tab}<VerticalStretch>{}</VerticalStretch>\r\n",
                    xml_bool(value)
                )
            })
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::TextColor => tooltip
            .text_color
            .as_ref()
            .map(|value| format!("{tab}<TextColor>{}</TextColor>\r\n", escape_xml_text(value)))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::Font => tooltip
            .font_xml
            .as_ref()
            .map(|value| format!("{tab}{value}\r\n"))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::Title => tooltip
            .title
            .as_ref()
            .map(|title| format_form_extended_tooltip_title_xml(title, indent))
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::GroupHorizontalAlign => tooltip
            .group_horizontal_align
            .map(|value| {
                format!(
                    "{tab}<GroupHorizontalAlign>{}</GroupHorizontalAlign>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::VerticalAlign => tooltip
            .vertical_align
            .map(|value| {
                format!(
                    "{tab}<VerticalAlign>{}</VerticalAlign>\r\n",
                    escape_xml_text(value)
                )
            })
            .unwrap_or_default(),
        FormExtendedTooltipXmlProperty::Events => {
            format_form_extended_tooltip_events_xml(&tooltip.events, indent)
        }
    }
}

fn format_form_extended_tooltip_title_xml(
    title: &FormExtendedTooltipTitle,
    indent: usize,
) -> String {
    let tab = "\t".repeat(indent);
    if title.values.is_empty() {
        return format!(
            "{tab}<Title formatted=\"{}\"/>\r\n",
            xml_bool(title.formatted)
        );
    }
    let mut xml = format!(
        "{tab}<Title formatted=\"{}\">\r\n",
        xml_bool(title.formatted)
    );
    for (lang, content) in &title.values {
        xml.push_str(&format!(
            "{tab}\t<v8:item>\r\n{tab}\t\t<v8:lang>{}</v8:lang>\r\n{tab}\t\t<v8:content>{}</v8:content>\r\n{tab}\t</v8:item>\r\n",
            escape_xml_element_text(lang),
            escape_xml_element_text(content)
        ));
    }
    xml.push_str(&format!("{tab}</Title>\r\n"));
    xml
}

fn format_form_extended_tooltip_events_xml(events: &[FormBodyEvent], indent: usize) -> String {
    if events.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<Events>\r\n");
    for event in events {
        xml.push_str(&format!(
            "{tab}\t<Event name=\"{}\">{}</Event>\r\n",
            escape_xml_text(&event.name),
            escape_xml_text(&event.handler)
        ));
    }
    xml.push_str(&format!("{tab}</Events>\r\n"));
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
        if item.presentation_present && item.presentation.is_empty() {
            xml.push_str(&format!("{tab}\t\t\t<Presentation/>\r\n"));
        } else if !item.presentation.is_empty() {
            xml.push_str(&format_form_localized_section(
                "Presentation",
                &item.presentation,
                indent + 3,
            ));
        }
        match &item.value {
            FormChoiceListValue::Boolean(value) => xml.push_str(&format!(
                "{tab}\t\t\t<Value xsi:type=\"xs:boolean\">{}</Value>\r\n",
                xml_bool(*value)
            )),
            FormChoiceListValue::Decimal(value) => xml.push_str(&format!(
                "{tab}\t\t\t<Value xsi:type=\"xs:decimal\">{}</Value>\r\n",
                escape_xml_text(value)
            )),
            FormChoiceListValue::Nil => {
                xml.push_str(&format!("{tab}\t\t\t<Value xsi:nil=\"true\"/>\r\n"))
            }
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

fn format_form_choice_parameter_links_xml(
    links: &[FormChoiceParameterLink],
    indent: usize,
) -> String {
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<ChoiceParameterLinks>\r\n");
    for link in links {
        xml.push_str(&format!(
            "{tab}\t<xr:Link>\r\n\
{tab}\t\t<xr:Name>{}</xr:Name>\r\n\
{tab}\t\t<xr:DataPath xsi:type=\"xs:string\">{}</xr:DataPath>\r\n\
{tab}\t\t<xr:ValueChange>{}</xr:ValueChange>\r\n\
{tab}\t</xr:Link>\r\n",
            escape_xml_text(&link.name),
            escape_xml_text(&link.data_path),
            escape_xml_text(link.value_change)
        ));
    }
    xml.push_str(&format!("{tab}</ChoiceParameterLinks>\r\n"));
    xml
}

fn format_form_type_link_xml(type_link: &FormTypeLink, indent: usize) -> String {
    let tab = "\t".repeat(indent);
    format!(
        "{tab}<TypeLink>\r\n\
{tab}\t<xr:DataPath>{}</xr:DataPath>\r\n\
{tab}\t<xr:LinkItem>{}</xr:LinkItem>\r\n\
{tab}</TypeLink>\r\n",
        escape_xml_text(&type_link.data_path),
        escape_xml_text(type_link.link_item)
    )
}

pub(super) fn should_emit_explicit_table_skip_on_input(item: &FormChildItem) -> bool {
    item.tag == "Table"
        && item.skip_on_input == Some(false)
        && (item.strict_table_schema
            || (!form_table_has_hierarchical_navigation(item)
                && (item.row_picture_data_path.is_some() || item.rows_picture_ref.is_some())))
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
        if attribute.main_attribute {
            xml.push_str("\t\t\t<MainAttribute>true</MainAttribute>\r\n");
        }
        if attribute.saved_data {
            xml.push_str("\t\t\t<SavedData>true</SavedData>\r\n");
        }
        if let Some(fill_check) = attribute.fill_check {
            xml.push_str(&format!(
                "\t\t\t<FillCheck>{}</FillCheck>\r\n",
                escape_xml_text(fill_check)
            ));
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
        if !attribute.columns.is_empty() || !attribute.additional_columns.is_empty() {
            xml.push_str("\t\t\t<Columns>\r\n");
            for column in &attribute.columns {
                xml.push_str(&format_form_attribute_column_xml(column, "\t\t\t\t"));
            }
            for additional in &attribute.additional_columns {
                if additional.columns.is_empty() {
                    xml.push_str(&format!(
                        "\t\t\t\t<AdditionalColumns table=\"{}\"/>\r\n",
                        escape_xml_text(&additional.table)
                    ));
                    continue;
                }
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
        if let Some(settings) = &attribute.settings {
            xml.push_str("\t\t\t<Settings xsi:type=\"DynamicList\">\r\n");
            if settings.manual_query || settings.manual_query_explicit {
                xml.push_str(&format!(
                    "\t\t\t\t<ManualQuery>{}</ManualQuery>\r\n",
                    xml_bool(settings.manual_query)
                ));
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
            if let Some(query_text) = &settings.query_text
                && !query_text.is_empty()
            {
                xml.push_str(&format!(
                    "\t\t\t\t<QueryText>{}</QueryText>\r\n",
                    escape_xml_element_text(query_text)
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
            escape_xml_element_text(lang),
            escape_xml_element_text(content)
        ));
    }
    xml.push_str(&format!("{tab}</{}>\r\n", name));
    xml
}

pub(super) fn format_form_title_section(item: &FormChildItem, indent: usize) -> String {
    if item.title.is_empty() && item.title_formatted.is_none() {
        return String::new();
    }
    if !matches!(item.tag, "LabelDecoration" | "PictureDecoration") {
        return format_form_localized_section("Title", &item.title, indent);
    }
    let tab = "\t".repeat(indent);
    if item.title.is_empty() {
        return format!(
            "{tab}<Title formatted=\"{}\"/>\r\n",
            xml_bool(item.title_formatted.unwrap_or(false))
        );
    }
    let mut xml = format!(
        "{tab}<Title formatted=\"{}\">\r\n",
        xml_bool(item.title_formatted.unwrap_or(false))
    );
    for (lang, content) in &item.title {
        xml.push_str(&format!(
            "{tab}\t<v8:item>\r\n{tab}\t\t<v8:lang>{}</v8:lang>\r\n{tab}\t\t<v8:content>{}</v8:content>\r\n{tab}\t</v8:item>\r\n",
            escape_xml_element_text(lang),
            escape_xml_element_text(content)
        ));
    }
    xml.push_str(&format!("{tab}</Title>\r\n"));
    xml
}

pub(super) fn format_form_command_interface_xml(
    command_interface: &FormCommandInterface,
) -> String {
    let mut xml = "\t<CommandInterface>\r\n".to_string();
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
            if let Some(attribute) = item.attribute.as_deref() {
                xml.push_str(&format!(
                    "\t\t\t\t<Attribute>{}</Attribute>\r\n",
                    escape_xml_text(attribute)
                ));
            }
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
            if let Some(visible) = &item.visible {
                xml.push_str("\t\t\t\t<Visible>\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t\t<xr:Common>{}</xr:Common>\r\n",
                    xml_bool(visible.common)
                ));
                for (role, value) in &visible.role_values {
                    xml.push_str(&format!(
                        "\t\t\t\t\t<xr:Value name=\"{}\">{}</xr:Value>\r\n",
                        escape_xml_text(role),
                        xml_bool(*value)
                    ));
                }
                xml.push_str("\t\t\t\t</Visible>\r\n");
            }
            xml.push_str("\t\t\t</Item>\r\n");
        }
        xml.push_str("\t\t</NavigationPanel>\r\n");
    }
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
            if let Some(attribute) = item.attribute.as_deref() {
                xml.push_str(&format!(
                    "\t\t\t\t<Attribute>{}</Attribute>\r\n",
                    escape_xml_text(attribute)
                ));
            }
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
            if let Some(visible) = &item.visible {
                xml.push_str("\t\t\t\t<Visible>\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t\t<xr:Common>{}</xr:Common>\r\n",
                    xml_bool(visible.common)
                ));
                for (role, value) in &visible.role_values {
                    xml.push_str(&format!(
                        "\t\t\t\t\t<xr:Value name=\"{}\">{}</xr:Value>\r\n",
                        escape_xml_text(role),
                        xml_bool(*value)
                    ));
                }
                xml.push_str("\t\t\t\t</Visible>\r\n");
            }
            xml.push_str("\t\t\t</Item>\r\n");
        }
        xml.push_str("\t\t</CommandBar>\r\n");
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
