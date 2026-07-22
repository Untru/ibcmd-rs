use super::*;

pub(super) struct CommandInterface {
    pub(super) commands_order: Vec<CommandInterfaceOrderEntry>,
    pub(super) commands_placement: Vec<CommandInterfacePlacementEntry>,
    pub(super) groups_order: Vec<String>,
    pub(super) commands_visibility: Vec<CommandInterfaceVisibilityEntry>,
    pub(super) subsystems_order: Vec<String>,
}

pub(super) struct CommandInterfacePlacementEntry {
    pub(super) name: String,
    pub(super) command_group: String,
    pub(super) placement: &'static str,
}

pub(super) struct CommandInterfaceOrderEntry {
    pub(super) name: String,
    pub(super) command_group: String,
}

pub(super) struct CommandInterfaceVisibilityEntry {
    pub(super) name: String,
    pub(super) common: bool,
}

pub(super) struct HomePageWorkArea {
    pub(super) template: &'static str,
    pub(super) left_column: Vec<HomePageWorkAreaItem>,
    pub(super) right_column: Vec<HomePageWorkAreaItem>,
}

pub(super) struct HomePageWorkAreaItem {
    pub(super) form: String,
    pub(super) height: String,
    pub(super) common: bool,
}

pub(super) struct ClientApplicationInterface {
    pub(super) top: Option<ClientApplicationInterfaceGroup>,
    pub(super) left: Option<ClientApplicationInterfaceGroup>,
    pub(super) panel_defs: Vec<String>,
}

pub(super) struct ClientApplicationInterfaceGroup {
    pub(super) id: Option<String>,
    pub(super) children: Vec<ClientApplicationInterfaceNode>,
}

pub(super) enum ClientApplicationInterfaceNode {
    Group(ClientApplicationInterfaceGroup),
    Panel { id: String, uuid: String },
}

pub(super) fn parse_command_interface_blob(
    bytes: &[u8],
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> Option<CommandInterface> {
    parse_command_interface_blob_with_subsystem_refs(
        bytes,
        command_refs,
        metadata_refs,
        &BTreeMap::new(),
    )
}

pub(super) fn parse_command_interface_blob_with_subsystem_refs(
    bytes: &[u8],
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<CommandInterface> {
    let inflated =
        crate::compiler::bodies::command_interface::decode_compatible_command_interface(bytes)
            .ok()?
            .plaintext()
            .ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }

    parse_command_interface_sectioned_fields(&fields, command_refs, metadata_refs, subsystem_refs)
}

pub(super) fn parse_command_interface_sectioned_fields(
    fields: &[&str],
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<CommandInterface> {
    let mut index = 1usize;

    // Each section starts with a 0/1 presence marker; present sections add a count.
    let count = parse_command_interface_section_count(fields, &mut index)?;
    let mut commands_visibility = Vec::with_capacity(count);
    for _ in 0..count {
        let name = parse_command_interface_command_name_field(
            fields.get(index)?,
            command_refs,
            metadata_refs,
            true,
        )?;
        index += 1;
        let common = parse_command_interface_common_flag(fields.get(index)?)?;
        index += 1;
        commands_visibility.push(CommandInterfaceVisibilityEntry { name, common });
    }

    let count = parse_command_interface_section_count(fields, &mut index)?;
    let mut commands_placement = Vec::with_capacity(count);
    for _ in 0..count {
        let name = parse_command_interface_command_name_field(
            fields.get(index)?,
            command_refs,
            metadata_refs,
            false,
        )?;
        index += 1;
        let command_group = command_interface_group_name(fields.get(index)?.trim(), metadata_refs);
        index += 1;
        let placement = command_interface_placement_name(fields.get(index)?.trim())?;
        index += 1;
        commands_placement.push(CommandInterfacePlacementEntry {
            name,
            command_group,
            placement,
        });
    }

    let count = parse_command_interface_section_count(fields, &mut index)?;
    let mut commands_order = Vec::with_capacity(count);
    for _ in 0..count {
        let command_group = command_interface_group_name(fields.get(index)?.trim(), metadata_refs);
        index += 1;
        let name = parse_command_interface_command_name_field(
            fields.get(index)?,
            command_refs,
            metadata_refs,
            true,
        )?;
        index += 1;
        commands_order.push(CommandInterfaceOrderEntry {
            name,
            command_group,
        });
    }

    let count = parse_command_interface_section_count(fields, &mut index)?;
    let mut subsystems_order = Vec::with_capacity(count);
    for _ in 0..count {
        let uuid = parse_non_zero_uuid(fields.get(index)?.trim())?;
        subsystems_order.push(command_interface_subsystem_name(
            &uuid,
            metadata_refs,
            subsystem_refs,
        ));
        index += 1;
    }

    let count = parse_command_interface_section_count(fields, &mut index)?;
    let mut groups_order = Vec::with_capacity(count);
    for _ in 0..count {
        groups_order.push(command_interface_group_name(
            fields.get(index)?.trim(),
            metadata_refs,
        ));
        index += 1;
    }

    if fields.get(index)?.trim() != "0" || index + 1 != fields.len() {
        return None;
    }

    Some(CommandInterface {
        commands_order,
        commands_placement,
        groups_order,
        commands_visibility,
        subsystems_order,
    })
}

pub(super) fn parse_command_interface_section_count(
    fields: &[&str],
    index: &mut usize,
) -> Option<usize> {
    let marker = fields.get(*index)?.trim();
    *index += 1;
    match marker {
        "0" => Some(0),
        "1" => {
            let count = fields.get(*index)?.trim().parse::<usize>().ok()?;
            *index += 1;
            Some(count)
        }
        _ => None,
    }
}

pub(super) fn parse_command_interface_command_name_field(
    field: &str,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    allow_bare_zero: bool,
) -> Option<String> {
    let command_ref = split_1c_braced_fields(field, 0)?;
    let code = command_ref.first()?.trim();
    if !code.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    match command_ref.as_slice() {
        [_] if allow_bare_zero && code == "0" => Some(code.to_string()),
        [_, uuid] if is_uuid_text(uuid.trim()) => Some(command_interface_command_name(
            code,
            uuid.trim(),
            command_refs,
            metadata_refs,
        )),
        _ => None,
    }
}

pub(super) fn command_interface_placement_name(code: &str) -> Option<&'static str> {
    match code {
        "0" => Some("Auto"),
        "1" => Some("Manual"),
        _ => None,
    }
}

pub(super) fn parse_command_interface_common_flag(value: &str) -> Option<bool> {
    if value.contains(r#"{"B",1}"#) {
        Some(true)
    } else if value.contains(r#"{"B",0}"#) {
        Some(false)
    } else {
        None
    }
}

pub(super) fn command_interface_group_name(
    uuid: &str,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> String {
    if let Some(name) = common_command_group_name(uuid) {
        return name.to_string();
    }
    metadata_refs
        .get(uuid)
        .filter(|reference| reference.kind == "CommandGroup")
        .map(|reference| format!("CommandGroup.{}", reference.name))
        .unwrap_or_else(|| uuid.to_string())
}

pub(super) fn command_interface_subsystem_name(
    uuid: &str,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> String {
    if let Some(name) = subsystem_refs
        .get(uuid)
        .and_then(subsystem_source_reference_name)
    {
        return name;
    }
    metadata_refs
        .get(uuid)
        .filter(|reference| reference.kind == "Subsystem")
        .map(|reference| format!("Subsystem.{}", reference.name))
        .unwrap_or_else(|| format!("Subsystem.{uuid}"))
}

pub(super) fn parse_home_page_work_area_blob(
    bytes: &[u8],
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<HomePageWorkArea> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    parse_home_page_work_area_text(text.trim_start_matches('\u{feff}'), form_refs)
}

pub(super) fn parse_home_page_work_area_text(
    text: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<HomePageWorkArea> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let template = home_page_work_area_template_name(fields.get(1)?.trim())?;
    let mut index = 2usize;
    let left_column = parse_home_page_work_area_column(&fields, &mut index, form_refs)?;
    let right_column = parse_home_page_work_area_column(&fields, &mut index, form_refs)?;

    Some(HomePageWorkArea {
        template,
        left_column,
        right_column,
    })
}

pub(super) fn home_page_work_area_template_name(code: &str) -> Option<&'static str> {
    match code {
        "2" => Some("TwoColumnsVariableWidth"),
        _ => None,
    }
}

pub(super) fn parse_home_page_work_area_column(
    fields: &[&str],
    index: &mut usize,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<Vec<HomePageWorkAreaItem>> {
    let count = fields.get(*index)?.trim().parse::<usize>().ok()?;
    *index += 1;
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        let item = parse_home_page_work_area_item(fields.get(*index)?, form_refs)?;
        *index += 1;
        items.push(item);
    }
    Some(items)
}

pub(super) fn parse_home_page_work_area_item(
    field: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<HomePageWorkAreaItem> {
    let fields = split_1c_braced_fields(field, 0)?;
    let form_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    let form_uuid = parse_non_zero_uuid(form_fields.get(1)?.trim())?;
    let form = form_refs
        .get(&form_uuid)
        .and_then(form_source_reference_name)
        .unwrap_or(form_uuid);
    let height = fields.get(2)?.trim().to_string();
    let common = parse_command_interface_common_flag(fields.get(3)?)?;

    Some(HomePageWorkAreaItem {
        form,
        height,
        common,
    })
}

pub(super) fn parse_client_application_interface_blob(
    bytes: &[u8],
) -> Option<ClientApplicationInterface> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    parse_client_application_interface_text(text.trim_start_matches('\u{feff}'))
}

pub(super) fn parse_client_application_interface_text(
    text: &str,
) -> Option<ClientApplicationInterface> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }

    let mut top = None;
    let mut left = None;
    let mut index = 1usize;
    while index < fields.len() {
        let Some(area_fields) = fields
            .get(index)
            .and_then(|field| split_1c_braced_fields(field, 0))
        else {
            break;
        };
        if area_fields.len() < 3 || area_fields.first().map(|value| value.trim()) != Some("0") {
            break;
        }
        let area_code = area_fields.get(1)?.trim();
        let group = parse_client_application_interface_area(area_fields.get(2)?)?;
        match area_code {
            "1" => top = group,
            "3" => left = group,
            _ => {}
        }
        index += 1;
    }

    let mut panel_defs = Vec::new();
    while index + 1 < fields.len() {
        let code = fields.get(index)?.trim();
        if code == "0" {
            break;
        }
        let panel_def_fields = split_1c_braced_fields(fields.get(index + 1)?, 0)?;
        let panel_uuid = parse_non_zero_uuid(panel_def_fields.first()?.trim())?;
        panel_defs.push(panel_uuid);
        index += 2;
    }

    Some(ClientApplicationInterface {
        top,
        left,
        panel_defs,
    })
}

pub(super) fn parse_client_application_interface_area(
    field: &str,
) -> Option<Option<ClientApplicationInterfaceGroup>> {
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 {
        return Some(None);
    }
    let group = fields
        .get(3)
        .and_then(|field| parse_client_application_interface_group(field, true))?;
    Some(Some(group))
}

pub(super) fn parse_client_application_interface_group(
    field: &str,
    with_id: bool,
) -> Option<ClientApplicationInterfaceGroup> {
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let id = if with_id {
        Some(parse_non_zero_uuid(fields.get(1)?.trim())?)
    } else {
        None
    };
    if fields.get(2)?.trim() != "0" {
        return None;
    }
    let children = parse_client_application_interface_children(fields.get(3)?)?;
    Some(ClientApplicationInterfaceGroup { id, children })
}

pub(super) fn parse_client_application_interface_children(
    field: &str,
) -> Option<Vec<ClientApplicationInterfaceNode>> {
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let mut children = Vec::with_capacity(count);
    let mut index = 2usize;
    for _ in 0..count {
        let _layout_marker = fields.get(index)?;
        index += 1;
        let child_fields = split_1c_braced_fields(fields.get(index)?, 0)?;
        index += 1;
        if child_fields.len() >= 4
            && child_fields.first().map(|value| value.trim()) == Some("0")
            && child_fields.get(2).map(|value| value.trim()) == Some("0")
        {
            children.push(ClientApplicationInterfaceNode::Group(
                parse_client_application_interface_group(fields.get(index - 1)?, true)?,
            ));
        } else {
            let id = parse_non_zero_uuid(child_fields.get(1)?.trim())?;
            let uuid = parse_non_zero_uuid(child_fields.get(2)?.trim())?;
            children.push(ClientApplicationInterfaceNode::Group(
                ClientApplicationInterfaceGroup {
                    id: None,
                    children: vec![ClientApplicationInterfaceNode::Panel { id, uuid }],
                },
            ));
        }
    }
    Some(children)
}

pub(super) fn command_interface_command_name(
    code: &str,
    uuid: &str,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> String {
    if let Some(name) = command_refs.get(uuid) {
        return name.clone();
    }
    if let Some(metadata) = metadata_refs.get(uuid) {
        if let Some(standard) = command_interface_standard_command_for_code(&metadata.kind, code) {
            return format!(
                "{}.{}.StandardCommand.{standard}",
                metadata.kind, metadata.name
            );
        }
    }

    format!("{code}:{uuid}")
}

pub(super) fn command_interface_standard_command(kind: &str) -> Option<&'static str> {
    match kind {
        "DataProcessor" | "Report" | "CommonForm" => Some("Open"),
        "AccountingRegister"
        | "AccumulationRegister"
        | "BusinessProcess"
        | "Catalog"
        | "ChartOfAccounts"
        | "ChartOfCalculationTypes"
        | "ChartOfCharacteristicTypes"
        | "Document"
        | "DocumentJournal"
        | "Enum"
        | "ExchangePlan"
        | "InformationRegister"
        | "Task" => Some("OpenList"),
        _ => None,
    }
}

pub(super) fn command_interface_standard_command_for_code(
    kind: &str,
    code: &str,
) -> Option<&'static str> {
    match (code, kind) {
        ("0" | "100", "Constant") => Some("Open"),
        ("0" | "100", "CalculationRegister") => Some("OpenList"),
        ("0" | "100", _) => command_interface_standard_command(kind),
        ("1", "BusinessProcess" | "Catalog" | "Document") => Some("Create"),
        ("1", "ChartOfCharacteristicTypes") => Some("CreateFolder"),
        ("2", "Catalog") => Some("CreateFolder"),
        ("2", "ChartOfCharacteristicTypes") => Some("Create"),
        _ => None,
    }
}

pub(super) fn format_command_interface_xml(command_interface: &CommandInterface) -> String {
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<CommandInterface xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
    );
    if !command_interface.commands_visibility.is_empty() {
        xml.push_str("\t<CommandsVisibility>\r\n");
        for entry in &command_interface.commands_visibility {
            xml.push_str(&format!(
                "\t\t<Command name=\"{}\">\r\n\
\t\t\t<Visibility>\r\n\
\t\t\t\t<xr:Common>{}</xr:Common>\r\n\
\t\t\t</Visibility>\r\n\
\t\t</Command>\r\n",
                escape_xml_text(&entry.name),
                xml_bool(entry.common)
            ));
        }
        xml.push_str("\t</CommandsVisibility>\r\n");
    }
    if !command_interface.commands_placement.is_empty() {
        xml.push_str("\t<CommandsPlacement>\r\n");
        for entry in &command_interface.commands_placement {
            xml.push_str(&format!(
                "\t\t<Command name=\"{}\">\r\n\
\t\t\t<CommandGroup>{}</CommandGroup>\r\n\
\t\t\t<Placement>{}</Placement>\r\n\
\t\t</Command>\r\n",
                escape_xml_text(&entry.name),
                escape_xml_text(&entry.command_group),
                entry.placement
            ));
        }
        xml.push_str("\t</CommandsPlacement>\r\n");
    }
    if !command_interface.commands_order.is_empty() {
        xml.push_str("\t<CommandsOrder>\r\n");
        for entry in &command_interface.commands_order {
            xml.push_str(&format!(
                "\t\t<Command name=\"{}\">\r\n\
\t\t\t<CommandGroup>{}</CommandGroup>\r\n\
\t\t</Command>\r\n",
                escape_xml_text(&entry.name),
                escape_xml_text(&entry.command_group)
            ));
        }
        xml.push_str("\t</CommandsOrder>\r\n");
    }
    if !command_interface.subsystems_order.is_empty() {
        xml.push_str("\t<SubsystemsOrder>\r\n");
        for subsystem in &command_interface.subsystems_order {
            xml.push_str(&format!(
                "\t\t<Subsystem>{}</Subsystem>\r\n",
                escape_xml_text(subsystem)
            ));
        }
        xml.push_str("\t</SubsystemsOrder>\r\n");
    }
    if !command_interface.groups_order.is_empty() {
        xml.push_str("\t<GroupsOrder>\r\n");
        for group in &command_interface.groups_order {
            xml.push_str(&format!(
                "\t\t<Group>{}</Group>\r\n",
                escape_xml_text(group)
            ));
        }
        xml.push_str("\t</GroupsOrder>\r\n");
    }
    xml.push_str("</CommandInterface>");
    xml
}

pub(super) fn format_home_page_work_area_xml(
    work_area: &HomePageWorkArea,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<HomePageWorkArea xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n",
        source_version.as_str()
    );
    xml.push_str(&format!(
        "\t<WorkingAreaTemplate>{}</WorkingAreaTemplate>\r\n",
        work_area.template
    ));
    push_home_page_work_area_column_xml(&mut xml, "LeftColumn", &work_area.left_column);
    push_home_page_work_area_column_xml(&mut xml, "RightColumn", &work_area.right_column);
    xml.push_str("</HomePageWorkArea>");
    xml
}

pub(super) fn push_home_page_work_area_column_xml(
    xml: &mut String,
    tag: &str,
    items: &[HomePageWorkAreaItem],
) {
    xml.push_str(&format!("\t<{tag}>\r\n"));
    for item in items {
        xml.push_str(&format!(
            "\t\t<Item>\r\n\
\t\t\t<Form>{}</Form>\r\n\
\t\t\t<Height>{}</Height>\r\n\
\t\t\t<Visibility>\r\n\
\t\t\t\t<xr:Common>{}</xr:Common>\r\n\
\t\t\t</Visibility>\r\n\
\t\t</Item>\r\n",
            escape_xml_element_text(&item.form),
            escape_xml_element_text(&item.height),
            xml_bool(item.common)
        ));
    }
    xml.push_str(&format!("\t</{tag}>\r\n"));
}

pub(super) fn format_client_application_interface_xml(
    interface: &ClientApplicationInterface,
) -> String {
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ClientApplicationInterface xmlns=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"InterfaceLayouter\">\r\n",
    );
    if let Some(group) = &interface.top {
        push_client_application_interface_area_xml(&mut xml, "top", group);
    }
    if let Some(group) = &interface.left {
        push_client_application_interface_area_xml(&mut xml, "left", group);
    }
    for panel_def in &interface.panel_defs {
        xml.push_str(&format!(
            "\t<panelDef id=\"{}\"/>\r\n",
            escape_xml_text(panel_def)
        ));
    }
    xml.push_str("</ClientApplicationInterface>");
    xml
}

pub(super) fn push_client_application_interface_area_xml(
    xml: &mut String,
    tag: &str,
    group: &ClientApplicationInterfaceGroup,
) {
    xml.push_str(&format!("\t<{tag}>\r\n"));
    push_client_application_interface_group_xml(xml, group, 2);
    xml.push_str(&format!("\t</{tag}>\r\n"));
}

pub(super) fn push_client_application_interface_group_xml(
    xml: &mut String,
    group: &ClientApplicationInterfaceGroup,
    indent: usize,
) {
    let tab = "\t".repeat(indent);
    if let Some(id) = &group.id {
        xml.push_str(&format!("{tab}<group id=\"{}\">\r\n", escape_xml_text(id)));
    } else {
        xml.push_str(&format!("{tab}<group>\r\n"));
    }
    for child in &group.children {
        match child {
            ClientApplicationInterfaceNode::Group(child_group) => {
                push_client_application_interface_group_xml(xml, child_group, indent + 1);
            }
            ClientApplicationInterfaceNode::Panel { id, uuid } => {
                let child_tab = "\t".repeat(indent + 1);
                xml.push_str(&format!(
                    "{child_tab}<panel id=\"{}\">\r\n\
{child_tab}\t<uuid>{}</uuid>\r\n\
{child_tab}</panel>\r\n",
                    escape_xml_text(id),
                    escape_xml_text(uuid)
                ));
            }
        }
    }
    xml.push_str(&format!("{tab}</group>\r\n"));
}
