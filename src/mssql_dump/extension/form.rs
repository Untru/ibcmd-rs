//! The forms of an extension.
//!
//! An extension keeps its forms in the ordinary form body. A form saved by an
//! older platform lacks members its items have since received: the platform
//! completes them in memory on load, taking the ids of the new items from the
//! id after the largest one of the form ([`upgrade_items`]). (The base form of
//! an adopted form and the call types of its handlers are written with the form
//! itself, from the facts `form_extension::form_adoption` reads;
//! [`add_call_types`] covers the adopted form that has no base form record.)

const EOL: &str = "\r\n";

/// Reads call types from the stored event blocks of an adopted form without
/// BaseForm. Commands retain the evidenced Before convention. Unknown event
/// blocks or ambiguous bindings fail instead of inventing Before.
pub(crate) fn add_call_types(xml: &str, layout: &str) -> anyhow::Result<Option<String>> {
    use super::super::{
        form_event_name_from_identifier, form_extension, scan_1c_braced_value,
        split_1c_braced_fields,
    };
    use anyhow::{Context, bail};

    let document = ibcmd_xml::XmlReader::from_slice(xml.as_bytes())
        .context("the adopted form XML is invalid")?;
    if document.root().children().iter().any(|node| matches!(node, ibcmd_xml::XmlNode::Element(element) if element.name().local() == "BaseForm")) {
        return Ok(None);
    }
    // A recognizable event head must have the complete evidenced tail. The
    // permissive legacy scanner alone would skip malformed/unknown tails.
    for (start, _) in layout.match_indices('{') {
        let Some(end) = scan_1c_braced_value(layout, start) else {
            continue;
        };
        let block = &layout[start..end];
        let Some(fields) = split_1c_braced_fields(block, 0) else {
            continue;
        };
        let Some(identifier) = fields.get(1) else {
            continue;
        };
        if form_event_name_from_identifier(identifier.trim().trim_matches('"')).is_some()
            && fields
                .first()
                .is_some_and(|count| count.trim().parse::<usize>().is_ok())
            && form_extension::parse_form_event_block(block).is_none()
        {
            bail!("unsupported or malformed adopted form event block for {identifier}");
        }
    }
    let call_types = form_extension::form_event_call_types(layout);
    let mut pending = vec![document.root()];
    while let Some(node) = pending.pop() {
        pending.extend(node.children().iter().filter_map(|child| match child {
            ibcmd_xml::XmlNode::Element(element) => Some(element),
            _ => None,
        }));
        if node.name().local() != "Event" {
            continue;
        }
        let attr = |name: &str| {
            node.attributes().iter().find_map(|attr| match attr.kind() {
                ibcmd_xml::AttributeKind::Ordinary(key) if key.raw() == name => Some(attr.value()),
                _ => None,
            })
        };
        let event = attr("name").context("form event has no name")?;
        let handler = node
            .children()
            .iter()
            .filter_map(|child| match child {
                ibcmd_xml::XmlNode::Text(text) => Some(text.value()),
                ibcmd_xml::XmlNode::CData(text) => Some(text.value()),
                _ => None,
            })
            .collect::<String>();
        let stored = call_types.get(event, &handler).with_context(|| {
            format!("adopted form event {event}/{handler} has no unambiguous stored call type")
        })?;
        if attr("callType").is_some_and(|xml_type| xml_type != stored) {
            bail!("adopted form event {event}/{handler} contradicts its stored call type");
        }
    }
    let rewritten = form_extension::with_action_call_types(&form_extension::with_event_call_types(
        xml,
        &call_types,
    ));
    Ok((rewritten != xml).then_some(rewritten))
}
/// The value of an XML attribute in the text of a start tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = tag.find(&marker)? + marker.len();
    let length = tag[start..].find('"')?;
    Some(&tag[start..start + length])
}

fn depth_of(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b'\t').count()
}

/// The start tag a line consists of, when it consists of nothing else.
struct StartTag<'a> {
    name: &'a str,
    attributes: &'a str,
    self_closed: bool,
}

fn start_tag(line: &str) -> Option<StartTag<'_>> {
    let text = line.trim_start_matches('\t');
    let inner = text.strip_prefix('<')?;
    if inner.starts_with('/') || inner.starts_with('?') || inner.contains('<') {
        return None;
    }
    let inner = inner.strip_suffix('>')?;
    let (inner, self_closed) = match inner.strip_suffix('/') {
        Some(inner) => (inner, true),
        None => (inner, false),
    };
    let name_end = inner.find(' ').unwrap_or(inner.len());
    Some(StartTag {
        name: &inner[..name_end],
        attributes: &inner[name_end..],
        self_closed,
    })
}

/// The last line of the element that starts at `start` (the line itself for an
/// element written on one line).
fn element_end(lines: &[&str], start: usize) -> usize {
    match start_tag(lines[start]) {
        Some(tag) if !tag.self_closed => {
            let depth = depth_of(lines[start]);
            (start + 1..lines.len())
                .find(|&index| {
                    depth_of(lines[index]) == depth
                        && lines[index].trim_start_matches('\t').starts_with("</")
                })
                .unwrap_or(lines.len() - 1)
        }
        // A self-closed tag or an element with its content on the line.
        _ => start,
    }
}

/// The tag name of the element written at `line`.
fn element_name(line: &str) -> Option<&str> {
    let text = line.trim_start_matches('\t').strip_prefix('<')?;
    if text.starts_with('/') || text.starts_with('?') {
        return None;
    }
    let end = text
        .find(|character: char| character == ' ' || character == '>' || character == '/')
        .unwrap_or(text.len());
    Some(&text[..end])
}

/// The id of the largest item and the types of the form attributes.
struct FormFacts {
    region_end: usize,
    max_id: i64,
    attribute_types: Vec<(String, String)>,
}

fn form_facts(lines: &[&str]) -> FormFacts {
    let region_end = lines
        .iter()
        .position(|line| {
            matches!(*line, "\t<Attributes>" | "\t<Commands>" | "\t<Parameters>")
                || line.starts_with("\t<BaseForm")
        })
        .unwrap_or(lines.len());
    let mut max_id = 0i64;
    for line in &lines[..region_end] {
        if let Some(tag) = start_tag(line)
            && let Some(id) = attribute(tag.attributes, "id").and_then(|id| id.parse::<i64>().ok())
        {
            max_id = max_id.max(id);
        }
    }
    let base_form = lines
        .iter()
        .position(|line| line.starts_with("\t<BaseForm"))
        .unwrap_or(lines.len());
    let mut attribute_types = Vec::new();
    let mut current = None::<String>;
    for line in &lines[region_end..base_form.max(region_end)] {
        if line.starts_with("\t\t<Attribute ")
            && let Some(tag) = start_tag(line)
        {
            current = attribute(tag.attributes, "name").map(str::to_owned);
        } else if let Some(name) = current.as_ref()
            && let Some(rest) = line.trim_start_matches('\t').strip_prefix("<v8:Type>")
            && let Some(type_name) = rest.strip_suffix("</v8:Type>")
        {
            attribute_types.push((name.clone(), type_name.to_owned()));
            current = None;
        }
    }
    FormFacts {
        region_end,
        max_id,
        attribute_types,
    }
}

const DYNAMIC_LIST: &str = "cfg:DynamicList";
const VALUE_LIST: &str = "v8:ValueListType";

/// The properties a dynamic-list table always writes, in the order it writes
/// them (between its own properties and its context menu).
const DYNAMIC_LIST_DEFAULTS: [&str; 13] = [
    "<AutoRefresh>false</AutoRefresh>",
    "<AutoRefreshPeriod>60</AutoRefreshPeriod>",
    "<Period>",
    "\t<v8:variant xsi:type=\"v8:StandardPeriodVariant\">Custom</v8:variant>",
    "\t<v8:startDate>0001-01-01T00:00:00</v8:startDate>",
    "\t<v8:endDate>0001-01-01T00:00:00</v8:endDate>",
    "</Period>",
    "<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>",
    "<RestoreCurrentRow>false</RestoreCurrentRow>",
    "<TopLevelParent xsi:nil=\"true\"/>",
    "<ShowRoot>true</ShowRoot>",
    "<AllowRootChoice>false</AllowRootChoice>",
    "<UpdateOnDataChange>Auto</UpdateOnDataChange>",
];

const DYNAMIC_LIST_LAST_DEFAULT: &str =
    "<AllowGettingCurrentRowURL>true</AllowGettingCurrentRowURL>";

struct Upgrade<'a> {
    lines: &'a [&'a str],
    facts: FormFacts,
    next_id: i64,
    out: Vec<String>,
    changed: bool,
}

impl<'a> Upgrade<'a> {
    fn take_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn push_line(&mut self, depth: usize, text: &str) {
        self.out.push(format!("{}{}", "\t".repeat(depth), text));
        self.changed = true;
    }

    fn push_extended_tooltip(&mut self, depth: usize, owner: &str) {
        let id = self.take_id();
        self.push_line(
            depth,
            &format!("<ExtendedTooltip name=\"{owner}РасширеннаяПодсказка\" id=\"{id}\"/>"),
        );
    }

    fn emit_range(&mut self, from: usize, to: usize) {
        let mut index = from;
        while index < to {
            let line = self.lines[index];
            let end = element_end(self.lines, index);
            match start_tag(line) {
                Some(tag) if tag.name == "Table" && !tag.self_closed && end < to => {
                    self.emit_table(index, end);
                    index = end + 1;
                }
                Some(tag) if tag.name == "CommandBar" && tag.self_closed => {
                    // An empty bar is expanded to hold the tooltip.
                    let depth = depth_of(line);
                    let name = attribute(tag.attributes, "name")
                        .unwrap_or_default()
                        .to_owned();
                    let attributes = tag.attributes.to_owned();
                    let indent = "\t".repeat(depth);
                    self.out.push(format!("{indent}<CommandBar{attributes}>"));
                    self.push_extended_tooltip(depth + 1, &name);
                    self.out.push(format!("{indent}</CommandBar>"));
                    index += 1;
                }
                Some(tag) if tag.name == "CommandBar" && end < to => {
                    self.emit_command_bar(index, end);
                    index = end + 1;
                }
                _ => {
                    self.out.push(line.to_owned());
                    index += 1;
                }
            }
        }
    }

    /// The direct children of the element at `start..=end`, as line ranges.
    fn children(&self, start: usize, end: usize) -> Vec<(usize, usize)> {
        let mut children = Vec::new();
        let mut index = start + 1;
        while index < end {
            let child_end = element_end(self.lines, index);
            children.push((index, child_end));
            index = child_end + 1;
        }
        children
    }

    fn emit_command_bar(&mut self, start: usize, end: usize) {
        let depth = depth_of(self.lines[start]);
        let Some(tag) = start_tag(self.lines[start]) else {
            return;
        };
        let name = attribute(tag.attributes, "name")
            .unwrap_or_default()
            .to_owned();
        let children = self.children(start, end);
        let has_tooltip = children
            .iter()
            .any(|(child, _)| element_name(self.lines[*child]) == Some("ExtendedTooltip"));
        self.out.push(self.lines[start].to_owned());
        let mut written = has_tooltip;
        for (child, child_end) in children {
            if !written && element_name(self.lines[child]) == Some("ChildItems") {
                self.push_extended_tooltip(depth + 1, &name);
                written = true;
            }
            self.emit_range(child, child_end + 1);
        }
        if !written {
            self.push_extended_tooltip(depth + 1, &name);
        }
        self.out.push(self.lines[end].to_owned());
    }

    fn emit_table(&mut self, start: usize, end: usize) {
        let depth = depth_of(self.lines[start]);
        let Some(tag) = start_tag(self.lines[start]) else {
            return;
        };
        let name = attribute(tag.attributes, "name")
            .unwrap_or_default()
            .to_owned();
        let children = self.children(start, end);
        let child_names: Vec<&str> = children
            .iter()
            .filter_map(|(child, _)| element_name(self.lines[*child]))
            .collect();
        let data_path = children.iter().find_map(|(child, _)| {
            let text = self.lines[*child].trim_start_matches('\t');
            text.strip_prefix("<DataPath>")?.strip_suffix("</DataPath>")
        });
        // A table over a dynamic list or a value list has no row filter; a
        // dynamic list also writes its refresh and choice settings.
        let source_type = data_path
            .filter(|path| !path.contains('.'))
            .and_then(|path| {
                self.facts
                    .attribute_types
                    .iter()
                    .find(|(attribute, _)| attribute == path)
                    .map(|(_, type_name)| type_name.as_str())
            });
        let is_dynamic_list = source_type == Some(DYNAMIC_LIST);
        let supports_row_filter = match data_path {
            None => false,
            Some(path) if path.contains('.') => true,
            Some(_) => source_type.is_some_and(|name| name != DYNAMIC_LIST && name != VALUE_LIST),
        };
        let needs_row_filter = supports_row_filter && !child_names.contains(&"RowFilter");
        let needs_dynamic_list_defaults = is_dynamic_list && !child_names.contains(&"AutoRefresh");
        let has_tooltip = child_names.contains(&"ExtendedTooltip");
        self.out.push(self.lines[start].to_owned());
        let mut context_menu_seen = false;
        for (child, child_end) in children {
            let child_name = element_name(self.lines[child]).unwrap_or_default();
            match child_name {
                "ContextMenu" if !context_menu_seen => {
                    context_menu_seen = true;
                    if needs_dynamic_list_defaults {
                        for line in DYNAMIC_LIST_DEFAULTS {
                            self.push_line(depth + 1, line);
                        }
                        self.push_line(depth + 1, DYNAMIC_LIST_LAST_DEFAULT);
                    }
                    if needs_row_filter {
                        self.push_line(depth + 1, "<RowFilter xsi:nil=\"true\"/>");
                    }
                    self.emit_range(child, child_end + 1);
                }
                "AutoCommandBar" => {
                    self.emit_range(child, child_end + 1);
                    if !has_tooltip {
                        self.push_extended_tooltip(depth + 1, &name);
                    }
                }
                "SearchStringAddition" | "ViewStatusAddition" | "SearchControlAddition" => {
                    self.emit_addition(child, child_end);
                }
                _ => self.emit_range(child, child_end + 1),
            }
        }
        self.out.push(self.lines[end].to_owned());
    }

    fn emit_addition(&mut self, start: usize, end: usize) {
        let depth = depth_of(self.lines[start]);
        let Some(tag) = start_tag(self.lines[start]) else {
            return;
        };
        let name = attribute(tag.attributes, "name")
            .unwrap_or_default()
            .to_owned();
        let children = self.children(start, end);
        let child_names: Vec<&str> = children
            .iter()
            .filter_map(|(child, _)| element_name(self.lines[*child]))
            .collect();
        let needs_items =
            !child_names.contains(&"ContextMenu") && !child_names.contains(&"ExtendedTooltip");
        self.out.push(self.lines[start].to_owned());
        for (child, child_end) in children {
            self.emit_range(child, child_end + 1);
            if needs_items && element_name(self.lines[child]) == Some("AdditionSource") {
                let id = self.take_id();
                self.push_line(
                    depth + 1,
                    &format!("<ContextMenu name=\"{name}КонтекстноеМеню\" id=\"{id}\"/>"),
                );
                self.push_extended_tooltip(depth + 1, &name);
            }
        }
        self.out.push(self.lines[end].to_owned());
    }
}

/// Completes the items of a form saved by an older platform the way the
/// platform does on load; `None` when the form needs nothing.
///
/// * a table without an extended tooltip gets one, and each of its search,
///   view-status and search-control additions gets a context menu and an
///   extended tooltip;
/// * a command bar without an extended tooltip gets one;
/// * a table over a dynamic list gets the list settings, any other table that
///   can filter its rows gets `<RowFilter xsi:nil="true"/>`.
///
/// The ids are taken in document order from the largest item id plus one.
pub(crate) fn upgrade_items(xml: &str) -> Option<String> {
    let lines: Vec<&str> = xml.split(EOL).collect();
    let facts = form_facts(&lines);
    let region_end = facts.region_end;
    let mut upgrade = Upgrade {
        lines: &lines,
        next_id: facts.max_id + 1,
        facts,
        out: Vec::with_capacity(lines.len() + 16),
        changed: false,
    };
    upgrade.emit_range(0, region_end);
    if !upgrade.changed {
        return None;
    }
    let mut out = upgrade.out;
    out.extend(lines[region_end..].iter().map(|line| (*line).to_owned()));
    Some(out.join(EOL))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crlf(text: &str) -> String {
        text.replace('\n', EOL)
    }

    #[test]
    fn a_command_bar_without_a_tooltip_gets_one() {
        let xml = crlf(
            "<Form>\n\t<ChildItems>\n\t\t<CommandBar name=\"Панель\" id=\"33\">\n\t\t\t<ChildItems>\n\t\t\t\t<Button name=\"Кнопка\" id=\"34\"/>\n\t\t\t</ChildItems>\n\t\t</CommandBar>\n\t</ChildItems>\n\t<Attributes>\n\t</Attributes>\n</Form>",
        );
        let upgraded = upgrade_items(&xml).unwrap();
        assert!(upgraded.contains(
            "\t\t\t<ExtendedTooltip name=\"ПанельРасширеннаяПодсказка\" id=\"35\"/>\r\n\t\t\t<ChildItems>"
        ));
    }

    #[test]
    fn a_table_is_completed_in_document_order() {
        let xml = crlf(
            "<Form>\n\t<ChildItems>\n\t\t<Table name=\"Т\" id=\"1\">\n\t\t\t<DataPath>Объект.Строки</DataPath>\n\t\t\t<ContextMenu name=\"ТКонтекстноеМеню\" id=\"2\"/>\n\t\t\t<AutoCommandBar name=\"ТКоманднаяПанель\" id=\"3\"/>\n\t\t\t<SearchStringAddition name=\"ТСтрокаПоиска\" id=\"4\">\n\t\t\t\t<AdditionSource>\n\t\t\t\t\t<Item>Т</Item>\n\t\t\t\t</AdditionSource>\n\t\t\t</SearchStringAddition>\n\t\t</Table>\n\t</ChildItems>\n\t<Attributes>\n\t</Attributes>\n</Form>",
        );
        let upgraded = upgrade_items(&xml).unwrap();
        let expected = crlf(
            "\t\t\t<DataPath>Объект.Строки</DataPath>\n\t\t\t<RowFilter xsi:nil=\"true\"/>\n\t\t\t<ContextMenu name=\"ТКонтекстноеМеню\" id=\"2\"/>\n\t\t\t<AutoCommandBar name=\"ТКоманднаяПанель\" id=\"3\"/>\n\t\t\t<ExtendedTooltip name=\"ТРасширеннаяПодсказка\" id=\"5\"/>\n\t\t\t<SearchStringAddition name=\"ТСтрокаПоиска\" id=\"4\">\n\t\t\t\t<AdditionSource>\n\t\t\t\t\t<Item>Т</Item>\n\t\t\t\t</AdditionSource>\n\t\t\t\t<ContextMenu name=\"ТСтрокаПоискаКонтекстноеМеню\" id=\"6\"/>\n\t\t\t\t<ExtendedTooltip name=\"ТСтрокаПоискаРасширеннаяПодсказка\" id=\"7\"/>\n\t\t\t</SearchStringAddition>",
        );
        assert!(upgraded.contains(&expected), "{upgraded}");
    }

    #[test]
    fn a_complete_form_is_left_alone() {
        let xml = crlf(
            "<Form>\n\t<ChildItems>\n\t\t<CommandBar name=\"Панель\" id=\"33\">\n\t\t\t<ExtendedTooltip name=\"ПанельРасширеннаяПодсказка\" id=\"35\"/>\n\t\t</CommandBar>\n\t</ChildItems>\n</Form>",
        );
        assert!(upgrade_items(&xml).is_none());
    }

    #[test]
    fn handlers_of_the_form_are_called_before() {
        let xml = crlf(
            "<Form>\n\t<Events>\n\t\t<Event name=\"OnCreateAtServer\">Обработчик</Event>\n\t</Events>\n\t<Commands>\n\t\t<Command name=\"К\" id=\"1\">\n\t\t\t<Action>Действие</Action>\n\t\t</Command>\n\t</Commands>\n</Form>",
        );
        let adjusted = add_call_types(&xml, "{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"Обработчик\",1,0,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,1}").unwrap().unwrap();
        assert!(
            adjusted.contains("<Event name=\"OnCreateAtServer\" callType=\"Before\">Обработчик")
        );
        assert!(adjusted.contains("<Action callType=\"Before\">Действие"));
    }

    /// A form with a base form record has its call types written with it.
    #[test]
    fn a_form_with_a_base_form_is_left_to_its_own_writer() {
        let xml = crlf(
            "<Form>\n\t<Events>\n\t\t<Event name=\"OnOpen\">Обработчик</Event>\n\t</Events>\n\t<BaseForm version=\"2.20\">\n\t</BaseForm>\n</Form>",
        );
        assert!(add_call_types(&xml, "").unwrap().is_none());
    }

    #[test]
    fn no_base_form_uses_native_before_after_and_override_codes() {
        let xml = crlf(
            "<Form>\n\t<Events>\n\t\t<Event name=\"OnCreateAtServer\">ПриСозданииНаСервере</Event>\n\t</Events>\n</Form>",
        );
        for (call_type, block) in [
            (
                "Before",
                include_str!(
                    "../../../tests/fixtures/native-evidence/extension-form-interceptors/before.block.txt"
                ),
            ),
            (
                "After",
                include_str!(
                    "../../../tests/fixtures/native-evidence/extension-form-interceptors/after.block.txt"
                ),
            ),
            (
                "Override",
                include_str!(
                    "../../../tests/fixtures/native-evidence/extension-form-interceptors/override.block.txt"
                ),
            ),
        ] {
            let actual = add_call_types(&xml, block.trim()).unwrap().unwrap();
            assert!(
                actual.contains(&format!("callType=\"{call_type}\"")),
                "{actual}"
            );
        }
    }

    #[test]
    fn malformed_and_unknown_stored_interceptors_are_refused() {
        const EVENT: &str = "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b";
        let xml = "<Form><Events><Event name=\"OnCreateAtServer\">Handler</Event></Events></Form>";
        for tail in ["3,1", "0,2,\"Handler\",3", "0,0", "0,2", "0,1,extra"] {
            let block = format!("{{1,{EVENT},\"Handler\",1,0,{EVENT},{tail}}}");
            assert!(add_call_types(xml, &block).is_err(), "{block}");
        }
        assert!(add_call_types(xml, "").is_err());
    }

    #[test]
    fn conflicting_xml_and_ambiguous_bindings_are_refused() {
        const EVENT: &str = "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b";
        let block = format!("{{1,{EVENT},\"Handler\",1,0,{EVENT},0,1}}");
        let xml = "<Form><Events><Event name=\"OnCreateAtServer\" callType=\"After\">Handler</Event></Events></Form>";
        assert!(add_call_types(xml, &block).is_err());
        let xml = xml.replace(" callType=\"After\"", "");
        let ambiguous = format!("{block}{{1,{EVENT},\"Handler\",1,0,{EVENT},2,1}}");
        assert!(add_call_types(&xml, &ambiguous).is_err());
    }

    #[test]
    fn one_handler_can_have_different_call_types_for_two_events() {
        let layout =
            "{2,OnOpen,\"Shared\",BeforeClose,\"\",1,0,OnOpen,0,1,BeforeClose,0,2,\"Shared\",1}";
        let xml = "<Form><Events><Event name=\"OnOpen\">Shared</Event><Event name=\"BeforeClose\">Shared</Event></Events><Commands><Command><Action>Run</Action></Command></Commands></Form>";
        let actual = add_call_types(xml, layout).unwrap().unwrap();
        assert!(actual.contains("name=\"OnOpen\" callType=\"Before\">Shared"));
        assert!(actual.contains("name=\"BeforeClose\" callType=\"After\">Shared"));
        assert!(actual.contains("<Action callType=\"Before\">Run</Action>"));
    }
}
