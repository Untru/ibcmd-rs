//! Canonical XML projection of typed legacy property views.

pub(crate) struct ConstantDetailsXml<'a> {
    pub mark_negatives: bool,
    pub mask: &'a str,
    pub multi_line: bool,
    pub extended_edit: bool,
    pub min_value: BoundXml<'a>,
    pub max_value: BoundXml<'a>,
    pub fill_checking: &'a str,
    pub choice_folders_and_items: &'a str,
    pub choice_parameter_links_xml: &'a str,
    pub choice_parameters_xml: &'a str,
    pub quick_choice: &'a str,
    pub choice_history_on_input: &'a str,
    pub data_lock_control_mode: &'a str,
    pub data_history: &'a str,
    pub update_immediately: bool,
    pub after_version: bool,
}

pub(crate) fn constant_details_xml(view: ConstantDetailsXml<'_>) -> String {
    format!(
        "\t\t\t<MarkNegatives>{}</MarkNegatives>\r\n\
\t\t\t{}\r\n\
\t\t\t<MultiLine>{}</MultiLine>\r\n\
\t\t\t<ExtendedEdit>{}</ExtendedEdit>\r\n\
\t\t\t{}\r\n\
\t\t\t{}\r\n\
\t\t\t<FillChecking>{}</FillChecking>\r\n\
\t\t\t<ChoiceFoldersAndItems>{}</ChoiceFoldersAndItems>\r\n\
{}\
\t\t\t{}\r\n\
\t\t\t<QuickChoice>{}</QuickChoice>\r\n\
\t\t\t<ChoiceForm/>\r\n\
\t\t\t<LinkByType/>\r\n\
\t\t\t<ChoiceHistoryOnInput>{}</ChoiceHistoryOnInput>\r\n\
\t\t\t<DataLockControlMode>{}</DataLockControlMode>\r\n\
\t\t\t<DataHistory>{}</DataHistory>\r\n\
\t\t\t<UpdateDataHistoryImmediatelyAfterWrite>{}</UpdateDataHistoryImmediatelyAfterWrite>\r\n\
\t\t\t<ExecuteAfterWriteDataHistoryVersionProcessing>{}</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
        view.mark_negatives,
        simple_property("Mask", view.mask),
        view.multi_line,
        view.extended_edit,
        bound_property("MinValue", view.min_value),
        bound_property("MaxValue", view.max_value),
        view.fill_checking,
        view.choice_folders_and_items,
        view.choice_parameter_links_xml,
        view.choice_parameters_xml,
        view.quick_choice,
        view.choice_history_on_input,
        view.data_lock_control_mode,
        view.data_history,
        view.update_immediately,
        view.after_version,
    )
}

pub(crate) struct CommonAttributeFlags {
    pub mark_negatives: bool,
    pub multi_line: bool,
    pub extended_edit: bool,
    pub fill_from_filling_value: bool,
}

pub(crate) fn common_attribute_prefix_xml(password_mode: bool) -> String {
    format!(
        "\t\t\t<PasswordMode>{password_mode}</PasswordMode>\r\n\t\t\t<Format/>\r\n\t\t\t<EditFormat/>\r\n"
    )
}

pub(crate) fn common_attribute_flags_xml(view: CommonAttributeFlags) -> String {
    format!(
        "\t\t\t<MarkNegatives>{}</MarkNegatives>\r\n\
\t\t\t<Mask/>\r\n\
\t\t\t<MultiLine>{}</MultiLine>\r\n\
\t\t\t<ExtendedEdit>{}</ExtendedEdit>\r\n\
\t\t\t<MinValue xsi:nil=\"true\"/>\r\n\
\t\t\t<MaxValue xsi:nil=\"true\"/>\r\n\
\t\t\t<FillFromFillingValue>{}</FillFromFillingValue>\r\n\
\t\t\t",
        view.mark_negatives, view.multi_line, view.extended_edit, view.fill_from_filling_value,
    )
}

pub(crate) fn insert_empty_calculation_register_children(xml: &mut String, kind: &str) {
    if kind == "CalculationRegister"
        && let Some(index) = xml.find("\t</CalculationRegister>")
    {
        xml.insert_str(index, "\t\t<ChildObjects/>\r\n");
    }
}

pub(crate) enum BoundXml<'a> {
    Nil,
    String(&'a str),
    Typed { xsi_type: &'a str, text: &'a str },
}

fn bound_property(name: &str, value: BoundXml<'_>) -> String {
    match value {
        BoundXml::Nil => format!("<{name} xsi:nil=\"true\"/>"),
        BoundXml::String(text) => format!(
            "<{name} xsi:type=\"xs:string\">{}</{name}>",
            escaped_text(text)
        ),
        BoundXml::Typed { xsi_type, text } => format!(
            "<{name} xsi:type=\"{xsi_type}\">{}</{name}>",
            escaped_text(text)
        ),
    }
}

fn simple_property(name: &str, text: &str) -> String {
    if text.is_empty() {
        format!("<{name}/>")
    } else {
        format!("<{name}>{}</{name}>", escaped_text(text))
    }
}

fn escaped_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
