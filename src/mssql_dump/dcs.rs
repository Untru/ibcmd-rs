use super::*;

const DCS_SCHEMA_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/schema";
const DCS_COMMON_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/common";
const DCS_CORE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/core";
const DCS_SETTINGS_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/settings";
const DATA_CORE_NS: &[u8] = b"http://v8.1c.ru/8.1/data/core";
const DATA_UI_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui";
const ENTERPRISE_NS: &[u8] = b"http://v8.1c.ru/8.1/data/enterprise";
const CURRENT_CONFIG_NS: &[u8] = b"http://v8.1c.ru/8.1/data/enterprise/current-config";
const STYLE_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/style";
const SYS_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/fonts/system";
const WEB_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/colors/web";
const WIN_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/colors/windows";
const XSI_NS: &[u8] = b"http://www.w3.org/2001/XMLSchema-instance";
const XS_NS: &[u8] = b"http://www.w3.org/2001/XMLSchema";
const DCS_SETTINGS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
const ENTERPRISE_URI: &str = "http://v8.1c.ru/8.1/data/enterprise";
const CURRENT_CONFIG_URI: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const SETTINGS_ROOT_UI_NAMESPACES: &str = " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\"";

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum DcsTypeResolution {
    KeepId,
    Type { qname: String },
    TypeSet { qname: String },
}

pub(super) type DcsTypeIndex = BTreeMap<String, DcsTypeResolution>;

pub(super) fn normalize_data_composition_schema_template_xml(
    inflated: &[u8],
    type_index: &DcsTypeIndex,
) -> Option<Vec<u8>> {
    let xml_start = find_bytes(inflated, b"<?xml")?;
    let text = std::str::from_utf8(&inflated[xml_start..]).ok()?;
    let documents = split_embedded_xml_documents(text);
    let schema_doc = documents.iter().find(|document| {
        document.contains("<SchemaFile") && document.contains("dataCompositionSchema")
    })?;
    let settings = documents
        .iter()
        .filter(|document| document.contains("<Settings") && document.contains(DCS_SETTINGS_URI))
        .filter_map(|document| canonicalize_data_composition_settings_document(document))
        .collect::<Vec<_>>();
    let mut xml = canonicalize_data_composition_schema_document(schema_doc)?;
    rewrite_data_composition_type_ids(&mut xml, type_index);
    insert_data_composition_settings(&mut xml, &settings)?;
    Some(xml.into_bytes())
}

fn rewrite_data_composition_type_ids(xml: &mut String, type_index: &DcsTypeIndex) {
    const OPEN: &str = "<v8:TypeId>";
    const CLOSE: &str = "</v8:TypeId>";
    const ANY_IB_REF_TYPE_ID: &str = "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63";
    const CFG_PREFIX: &str = "cfg:";
    let mut rewritten = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    while let Some(relative_start) = xml[cursor..].find(OPEN) {
        let start = cursor + relative_start;
        let value_start = start + OPEN.len();
        let Some(relative_end) = xml[value_start..].find(CLOSE) else {
            break;
        };
        let value_end = value_start + relative_end;
        let type_id = xml[value_start..value_end].trim();
        let dcs_cfg_prefix = data_composition_current_config_prefix(xml, start);
        let replacement = if type_id.eq_ignore_ascii_case(ANY_IB_REF_TYPE_ID) {
            Some(format!(
                "<v8:TypeSet xmlns:{dcs_cfg_prefix}=\"{CURRENT_CONFIG_URI}\">{dcs_cfg_prefix}:AnyIBRef</v8:TypeSet>"
            ))
        } else {
            type_index
                .get(&type_id.to_ascii_lowercase())
                .and_then(|resolution| match resolution {
                    DcsTypeResolution::KeepId => None,
                    DcsTypeResolution::Type { qname } => {
                        let reference = qname.strip_prefix(CFG_PREFIX)?;
                        Some(format!(
                            "<v8:Type xmlns:{dcs_cfg_prefix}=\"{CURRENT_CONFIG_URI}\">{dcs_cfg_prefix}:{}</v8:Type>",
                            escape_xml_text(reference)
                        ))
                    }
                    DcsTypeResolution::TypeSet { qname } => {
                        let reference = qname.strip_prefix(CFG_PREFIX)?;
                        Some(format!(
                            "<v8:TypeSet xmlns:{dcs_cfg_prefix}=\"{CURRENT_CONFIG_URI}\">{dcs_cfg_prefix}:{}</v8:TypeSet>",
                            escape_xml_text(reference)
                        ))
                    }
                })
        };
        if let Some(replacement) = replacement {
            rewritten.push_str(&xml[cursor..start]);
            rewritten.push_str(&replacement);
            cursor = value_end + CLOSE.len();
        } else {
            rewritten.push_str(&xml[cursor..value_end + CLOSE.len()]);
            cursor = value_end + CLOSE.len();
        }
    }
    if cursor == 0 {
        return;
    }
    rewritten.push_str(&xml[cursor..]);
    *xml = rewritten;
}

#[derive(Debug)]
struct DcsXmlStackEntry<'a> {
    name: &'a str,
    xsi_type: Option<&'a str>,
}

fn data_composition_current_config_prefix(xml: &str, position: usize) -> String {
    let mut stack = Vec::<DcsXmlStackEntry<'_>>::new();
    let mut cursor = 0usize;
    let bytes = xml.as_bytes();
    let limit = position.min(xml.len());
    while cursor < limit {
        let Some(relative_start) = xml[cursor..limit].find('<') else {
            break;
        };
        let start = cursor + relative_start;
        if xml[start..].starts_with("<!--") {
            let Some(relative_end) = xml[start + 4..limit].find("-->") else {
                break;
            };
            cursor = start + 4 + relative_end + 3;
            continue;
        }
        if xml[start..].starts_with("<![CDATA[") {
            let Some(relative_end) = xml[start + 9..limit].find("]]>") else {
                break;
            };
            cursor = start + 9 + relative_end + 3;
            continue;
        }
        let Some(end) = find_xml_tag_end(bytes, start + 1, limit) else {
            break;
        };
        let tag = xml[start + 1..end].trim();
        if tag.starts_with('?') || tag.starts_with('!') {
            cursor = end + 1;
            continue;
        }
        if let Some(end_name) = tag.strip_prefix('/') {
            let end_name = dcs_xml_tag_name(end_name);
            if let Some(index) = stack.iter().rposition(|entry| entry.name == end_name) {
                stack.truncate(index);
            }
        } else {
            let self_closing = tag.ends_with('/');
            let tag = tag.strip_suffix('/').unwrap_or(tag).trim_end();
            let name = dcs_xml_tag_name(tag);
            if !self_closing && !name.is_empty() {
                stack.push(DcsXmlStackEntry {
                    name,
                    xsi_type: dcs_xml_attribute_value(tag, "xsi:type"),
                });
            }
        }
        cursor = end + 1;
    }

    let base = if stack
        .iter()
        .any(|entry| matches!(entry.name, "parameter" | "calculatedField"))
    {
        4
    } else if stack
        .iter()
        .any(|entry| entry.name == "item" && entry.xsi_type == Some("DataSetObject"))
    {
        6
    } else {
        5
    };
    let nested_schema_depth = stack
        .iter()
        .filter(|entry| entry.name == "nestedSchema")
        .count();
    format!("d{}p1", base + 2 * nested_schema_depth)
}

fn find_xml_tag_end(bytes: &[u8], mut cursor: usize, limit: usize) -> Option<usize> {
    let mut quote = None::<u8>;
    while cursor < limit {
        let byte = bytes[cursor];
        if quote == Some(byte) {
            quote = None;
        } else if quote.is_none() && matches!(byte, b'"' | b'\'') {
            quote = Some(byte);
        } else if quote.is_none() && byte == b'>' {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn dcs_xml_tag_name(tag: &str) -> &str {
    tag.split_whitespace().next().unwrap_or("")
}

fn dcs_xml_attribute_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        let index = rest.find(name)?;
        let candidate = &rest[index + name.len()..];
        let before_is_name_char = index > 0 && rest.as_bytes()[index - 1].is_ascii_alphanumeric()
            || index > 0 && matches!(rest.as_bytes()[index - 1], b':' | b'_' | b'-');
        if before_is_name_char || !candidate.trim_start().starts_with('=') {
            rest = &candidate[1.min(candidate.len())..];
            continue;
        }
        let after_equals = candidate.trim_start().strip_prefix('=')?.trim_start();
        let quote = after_equals.as_bytes().first().copied()?;
        if !matches!(quote, b'"' | b'\'') {
            return None;
        }
        let value_start = 1;
        let value_end = after_equals[value_start..].find(quote as char)? + value_start;
        return Some(&after_equals[value_start..value_end]);
    }
}

pub(super) fn extract_ws_definition_xml(inflated: &[u8]) -> Option<Vec<u8>> {
    let xml_start = find_bytes(inflated, b"<?xml")?;
    let xml = &inflated[xml_start..];
    let mut content = Vec::with_capacity(3 + xml.len());
    content.extend_from_slice(b"\xEF\xBB\xBF");
    content.extend_from_slice(xml);
    Some(content)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn split_embedded_xml_documents(text: &str) -> Vec<&str> {
    let mut starts = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find("<?xml") {
        starts.push(cursor + offset);
        cursor += offset + "<?xml".len();
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(text.len());
            text[*start..end].trim_matches('\u{feff}').trim()
        })
        .filter(|document| !document.is_empty())
        .collect()
}

fn canonicalize_data_composition_schema_document(document: &str) -> Option<String> {
    let mut writer = DataCompositionXmlWriter::new();
    writer.fixed_decl_and_schema_root();
    let root_len = writer.output.len();
    writer.write_document(document, DataCompositionDocumentMode::Schema)?;
    let body = writer.output[root_len..].to_string();
    writer.output.truncate(root_len);
    writer
        .output
        .push_str(&normalize_data_composition_schema_body_indent(&body));
    writer.output.push_str("\r\n</DataCompositionSchema>");
    Some(writer.output)
}

fn canonicalize_data_composition_settings_document(document: &str) -> Option<String> {
    let mut writer = DataCompositionXmlWriter::new();
    writer.write_document(document, DataCompositionDocumentMode::Settings)?;
    let settings = writer
        .output
        .trim_start_matches(['\r', '\n', '\t'])
        .to_string();
    Some(indent_data_composition_settings(&settings))
}

fn insert_data_composition_settings(xml: &mut String, settings: &[String]) -> Option<()> {
    let offsets = direct_settings_variant_closing_offsets(xml)?;
    if offsets.len() != settings.len() {
        return None;
    }
    for (offset, settings_block) in offsets.into_iter().zip(settings.iter()).rev() {
        xml.insert_str(offset, settings_block);
    }
    Some(())
}

fn direct_settings_variant_closing_offsets(xml: &str) -> Option<Vec<usize>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    let mut offsets = Vec::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                stack.push((
                    namespace_ref(&namespace).map(<[u8]>::to_vec),
                    local.as_ref().to_vec(),
                ));
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                if stack.len() == 1
                    && namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                    && local.as_ref() == b"settingsVariant"
                {
                    return None;
                }
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                if stack.len() == 2
                    && stack.first().is_some_and(|(namespace, local)| {
                        namespace.as_deref() == Some(DCS_SCHEMA_NS)
                            && local.as_slice() == b"DataCompositionSchema"
                    })
                    && namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                    && local.as_ref() == b"settingsVariant"
                {
                    let end_tag_len = event.name().as_ref().len() + 3;
                    let position = usize::try_from(reader.buffer_position()).ok()?;
                    offsets.push(position.checked_sub(end_tag_len)?);
                }
                let (open_namespace, open_local) = stack.pop()?;
                if open_namespace.as_deref() != namespace_ref(&namespace)
                    || open_local.as_slice() != local.as_ref()
                {
                    return None;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.is_empty() {
        Some(offsets)
    } else {
        None
    }
}

fn normalize_data_composition_schema_body_indent(body: &str) -> String {
    let body = body.strip_prefix("\r\n\r\n").unwrap_or(body);
    body.trim_end_matches(['\r', '\n', '\t']).to_string()
}

fn deindent_lines_by_one_tab(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| line.strip_prefix('\t').unwrap_or(line))
        .collect()
}

fn is_xml_whitespace(text: &str) -> bool {
    text.bytes()
        .all(|byte| matches!(byte, b'\r' | b'\n' | b'\t' | b' '))
}

fn indent_data_composition_settings(settings: &str) -> String {
    let mut indented = String::from("\r\n");
    for line in settings.split_inclusive('\n') {
        indented.push_str("\t\t");
        indented.push_str(line);
    }
    indented
}

enum DataCompositionDocumentMode {
    Schema,
    Settings,
}

#[derive(Debug)]
struct DcsElementFrame {
    namespace: Option<Vec<u8>>,
    local: Vec<u8>,
    xsi_type_local: Option<String>,
    dynamic_prefixes: Vec<String>,
}

#[derive(Debug)]
struct DcsExpandedQName {
    namespace: Option<Vec<u8>>,
    local: String,
}

#[derive(Debug)]
struct DcsRenderedQName {
    value: String,
    declaration: Option<(String, &'static str)>,
}

struct DataCompositionXmlWriter {
    output: String,
    skip_depth: usize,
    element_stack: Vec<DcsElementFrame>,
}

impl DataCompositionXmlWriter {
    fn new() -> Self {
        Self {
            output: String::new(),
            skip_depth: 0,
            element_stack: Vec::new(),
        }
    }

    fn fixed_decl_and_schema_root(&mut self) {
        self.output.push_str(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
        );
    }

    fn write_document(&mut self, document: &str, mode: DataCompositionDocumentMode) -> Option<()> {
        let mut reader = NsReader::from_str(document);
        reader.config_mut().trim_text(false);
        loop {
            match reader.read_event().ok()? {
                Event::Start(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    let local = local.as_ref();
                    if self.should_skip(namespace_ref(&namespace), local, &mode) {
                        continue;
                    }
                    if matches!(mode, DataCompositionDocumentMode::Schema)
                        && namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                        && local == b"dataCompositionSchema"
                    {
                        continue;
                    }
                    if self.skip_depth == 0 {
                        let dynamic_prefixes = self.write_start_tag(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local,
                            false,
                            &mode,
                        )?;
                        self.element_stack.push(data_composition_element_frame(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local,
                            dynamic_prefixes,
                        )?);
                    }
                }
                Event::Empty(event) => {
                    if self.skip_depth == 0 {
                        let (namespace, local) = reader.resolve_element(event.name());
                        self.write_start_tag(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local.as_ref(),
                            true,
                            &mode,
                        )?;
                    }
                }
                Event::End(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    let local = local.as_ref();
                    if self.should_skip(namespace_ref(&namespace), local, &mode) {
                        continue;
                    }
                    if matches!(mode, DataCompositionDocumentMode::Schema)
                        && namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                        && local == b"dataCompositionSchema"
                    {
                        continue;
                    }
                    let name = if matches!(mode, DataCompositionDocumentMode::Settings)
                        && namespace_ref(&namespace) == Some(DCS_SETTINGS_NS)
                        && local == b"Settings"
                    {
                        "dcsset:settings".to_string()
                    } else {
                        canonical_data_composition_name(namespace_ref(&namespace), local)?
                    };
                    self.output.push_str("</");
                    self.output.push_str(&name);
                    self.output.push('>');
                    let frame = self.element_stack.pop()?;
                    if frame.namespace.as_deref() != namespace_ref(&namespace)
                        || frame.local.as_slice() != local
                    {
                        return None;
                    }
                }
                Event::Text(event) => {
                    if self.skip_depth == 0 {
                        self.write_text(&reader, &event, &mode)?;
                    }
                }
                Event::CData(event) => {
                    if self.skip_depth == 0 {
                        self.output.push_str("<![CDATA[");
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push_str("]]>");
                    }
                }
                Event::Comment(event) => {
                    if self.skip_depth == 0 {
                        self.output.push_str("<!--");
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push_str("-->");
                    }
                }
                Event::GeneralRef(event) => {
                    if self.skip_depth == 0 {
                        self.output.push('&');
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push(';');
                    }
                }
                Event::Decl(_) => {}
                Event::Eof => break,
                _ => {}
            }
        }
        self.element_stack.is_empty().then_some(())
    }

    fn should_skip(
        &self,
        namespace: Option<&[u8]>,
        local: &[u8],
        mode: &DataCompositionDocumentMode,
    ) -> bool {
        matches!(mode, DataCompositionDocumentMode::Schema)
            && namespace.is_none()
            && local == b"SchemaFile"
    }

    fn write_start_tag(
        &mut self,
        reader: &NsReader<&[u8]>,
        event: &quick_xml::events::BytesStart<'_>,
        namespace: Option<&[u8]>,
        local: &[u8],
        empty: bool,
        mode: &DataCompositionDocumentMode,
    ) -> Option<Vec<String>> {
        let is_settings_root = matches!(mode, DataCompositionDocumentMode::Settings)
            && namespace == Some(DCS_SETTINGS_NS)
            && local == b"Settings";
        let is_inline_settings_root = local == b"settings"
            && self.element_stack.last().is_some_and(|parent| {
                (namespace == Some(DCS_SETTINGS_NS)
                    && parent.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                    && parent.local.as_slice() == b"settingsVariant")
                    || (namespace == Some(DCS_SCHEMA_NS)
                        && parent.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                        && parent.local.as_slice() == b"nestedSchema")
            });
        let name = if is_settings_root {
            "dcsset:settings".to_string()
        } else {
            canonical_data_composition_name(namespace, local)?
        };
        let mut rendered_attributes = Vec::<(String, String)>::new();
        let mut dynamic_declarations = Vec::<(String, &'static str)>::new();
        if namespace == Some(DATA_CORE_NS)
            && matches!(local, b"Type" | b"TypeSet")
            && event_declares_namespace(event, CURRENT_CONFIG_NS)
        {
            push_dynamic_declaration(
                &mut dynamic_declarations,
                self.current_config_prefix(),
                CURRENT_CONFIG_URI,
            )?;
        }
        for attribute in event.attributes().with_checks(false) {
            let attribute = attribute.ok()?;
            if is_xmlns_attribute(attribute.key.as_ref()) {
                continue;
            }
            let (attr_namespace, attr_local) = reader.resolve_attribute(attribute.key);
            let attr_name = canonical_data_composition_attr_name(
                namespace_ref(&attr_namespace),
                attr_local.as_ref(),
            )?;
            let value = attribute
                .decode_and_unescape_value(reader.decoder())
                .ok()?
                .into_owned();
            let rendered = if attr_name == "xsi:type" {
                self.render_xsi_type(reader, &value, namespace, local)
            } else {
                None
            };
            let value = if let Some(rendered) = rendered {
                if let Some((prefix, uri)) = rendered.declaration {
                    push_dynamic_declaration(&mut dynamic_declarations, prefix, uri)?;
                }
                rendered.value
            } else {
                canonical_data_composition_attr_value(&attr_name, &value, namespace)
            };
            rendered_attributes.push((attr_name, value));
        }
        self.output.push('<');
        self.output.push_str(&name);
        if is_settings_root || is_inline_settings_root {
            self.output.push_str(SETTINGS_ROOT_UI_NAMESPACES);
        }
        for (prefix, uri) in &dynamic_declarations {
            self.output.push_str(" xmlns:");
            self.output.push_str(prefix);
            self.output.push_str("=\"");
            self.output.push_str(uri);
            self.output.push('"');
        }
        for (attr_name, value) in rendered_attributes {
            self.output.push(' ');
            self.output.push_str(&attr_name);
            self.output.push_str("=\"");
            self.output.push_str(&escape_xml_text(&value));
            self.output.push('"');
        }
        if empty {
            self.output.push_str("/>");
        } else {
            self.output.push('>');
        }
        Some(
            dynamic_declarations
                .into_iter()
                .map(|(prefix, _)| prefix)
                .collect(),
        )
    }

    fn write_text(
        &mut self,
        reader: &NsReader<&[u8]>,
        event: &quick_xml::events::BytesText<'_>,
        mode: &DataCompositionDocumentMode,
    ) -> Option<()> {
        let text = std::str::from_utf8(event.as_ref()).ok()?;
        if matches!(mode, DataCompositionDocumentMode::Schema) && is_xml_whitespace(text) {
            self.output.push_str(&deindent_lines_by_one_tab(text));
            return Some(());
        }
        let is_qname_text = self.element_stack.last().is_some_and(|frame| {
            frame.namespace.as_deref() == Some(DATA_CORE_NS)
                && matches!(frame.local.as_slice(), b"Type" | b"TypeSet")
        });
        if is_qname_text {
            let value = text.trim();
            if !value.is_empty()
                && let Some(rendered) = self.render_lexical_qname(
                    reader,
                    value,
                    Some(DATA_CORE_NS),
                    self.element_stack.last()?.local.as_slice(),
                )
            {
                if let Some((prefix, _)) = &rendered.declaration
                    && !self.element_stack.last()?.dynamic_prefixes.contains(prefix)
                {
                    return None;
                }
                let value_start = text.find(value)?;
                self.output.push_str(&text[..value_start]);
                self.output.push_str(&escape_xml_text(&rendered.value));
                self.output.push_str(&text[value_start + value.len()..]);
                return Some(());
            }
        }
        self.output.push_str(text);
        Some(())
    }

    fn render_lexical_qname(
        &self,
        reader: &NsReader<&[u8]>,
        value: &str,
        element_namespace: Option<&[u8]>,
        element_local: &[u8],
    ) -> Option<DcsRenderedQName> {
        let mut expanded = resolve_data_composition_qname(reader, value)?;
        if !value.contains(':')
            && matches!(
                element_namespace,
                Some(DCS_CORE_NS | DCS_SETTINGS_NS | DATA_CORE_NS)
            )
        {
            expanded.namespace = element_namespace.map(<[u8]>::to_vec);
        }
        self.render_expanded_qname(expanded, element_local)
    }

    fn render_xsi_type(
        &self,
        reader: &NsReader<&[u8]>,
        value: &str,
        element_namespace: Option<&[u8]>,
        element_local: &[u8],
    ) -> Option<DcsRenderedQName> {
        if value.contains(':') {
            return self.render_lexical_qname(reader, value, element_namespace, element_local);
        }
        let namespace = if is_data_core_xsi_type(value) {
            Some(DATA_CORE_NS)
        } else if value == "Field" {
            Some(DCS_CORE_NS)
        } else if is_dcs_settings_xsi_type(value) {
            Some(DCS_SETTINGS_NS)
        } else if matches!(element_namespace, Some(DCS_CORE_NS | DCS_SETTINGS_NS)) {
            element_namespace
        } else {
            return self.render_lexical_qname(reader, value, element_namespace, element_local);
        };
        self.render_expanded_qname(
            DcsExpandedQName {
                namespace: namespace.map(<[u8]>::to_vec),
                local: value.to_string(),
            },
            element_local,
        )
    }

    fn render_expanded_qname(
        &self,
        expanded: DcsExpandedQName,
        element_local: &[u8],
    ) -> Option<DcsRenderedQName> {
        let namespace = expanded.namespace.as_deref();
        let fixed_prefix = match namespace {
            None | Some(DCS_SCHEMA_NS) => Some(None),
            Some(DCS_COMMON_NS) => Some(Some("dcscom")),
            Some(DCS_CORE_NS) => Some(Some("dcscor")),
            Some(DCS_SETTINGS_NS) => Some(Some("dcsset")),
            Some(DATA_CORE_NS) => Some(Some("v8")),
            Some(DATA_UI_NS) => Some(Some("v8ui")),
            Some(STYLE_NS) => Some(Some("style")),
            Some(SYS_NS) => Some(Some("sys")),
            Some(WEB_NS) => Some(Some("web")),
            Some(WIN_NS) => Some(Some("win")),
            Some(XSI_NS) => Some(Some("xsi")),
            Some(XS_NS) => Some(Some("xs")),
            _ => None,
        };
        if let Some(prefix) = fixed_prefix {
            let value = prefix
                .map(|prefix| format!("{prefix}:{}", expanded.local))
                .unwrap_or(expanded.local);
            return Some(DcsRenderedQName {
                value,
                declaration: None,
            });
        }
        let (prefix, uri) = match namespace {
            Some(CURRENT_CONFIG_NS) => (self.current_config_prefix(), CURRENT_CONFIG_URI),
            Some(ENTERPRISE_NS) => (self.enterprise_prefix(element_local), ENTERPRISE_URI),
            _ => return None,
        };
        Some(DcsRenderedQName {
            value: format!("{prefix}:{}", expanded.local),
            declaration: Some((prefix, uri)),
        })
    }

    fn current_config_prefix(&self) -> String {
        let base = if self.has_parameter_ancestor() {
            4
        } else if self.element_stack.iter().any(|frame| {
            frame.local.as_slice() == b"item"
                && frame.xsi_type_local.as_deref() == Some("DataSetObject")
        }) {
            6
        } else {
            5
        };
        self.scope_prefix(base)
    }

    fn enterprise_prefix(&self, element_local: &[u8]) -> String {
        let base = if element_local != b"mode" {
            5
        } else if self.has_parameter_ancestor() {
            7
        } else {
            8
        };
        self.scope_prefix(base)
    }

    fn has_parameter_ancestor(&self) -> bool {
        self.element_stack.iter().any(|frame| {
            frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                && matches!(frame.local.as_slice(), b"parameter" | b"calculatedField")
        })
    }

    fn scope_prefix(&self, base: usize) -> String {
        let nested_schema_depth = self
            .element_stack
            .iter()
            .filter(|frame| {
                frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                    && frame.local.as_slice() == b"nestedSchema"
            })
            .count();
        format!("d{}p1", base + 2 * nested_schema_depth)
    }
}

fn data_composition_element_frame(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    namespace: Option<&[u8]>,
    local: &[u8],
    dynamic_prefixes: Vec<String>,
) -> Option<DcsElementFrame> {
    let mut xsi_type_local = None;
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (attr_namespace, attr_local) = reader.resolve_attribute(attribute.key);
        if namespace_ref(&attr_namespace) == Some(XSI_NS) && attr_local.as_ref() == b"type" {
            let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
            xsi_type_local = Some(
                value
                    .rsplit_once(':')
                    .map(|(_, local)| local)
                    .unwrap_or(value.as_ref())
                    .to_string(),
            );
            break;
        }
    }
    Some(DcsElementFrame {
        namespace: namespace.map(<[u8]>::to_vec),
        local: local.to_vec(),
        xsi_type_local,
        dynamic_prefixes,
    })
}

fn resolve_data_composition_qname(
    reader: &NsReader<&[u8]>,
    value: &str,
) -> Option<DcsExpandedQName> {
    let (namespace, local) = reader.resolve(quick_xml::name::QName(value.as_bytes()), false);
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.0.to_vec()),
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(_) => return None,
    };
    Some(DcsExpandedQName {
        namespace,
        local: std::str::from_utf8(local.as_ref()).ok()?.to_string(),
    })
}

fn event_declares_namespace(event: &quick_xml::events::BytesStart<'_>, namespace: &[u8]) -> bool {
    event
        .attributes()
        .with_checks(false)
        .flatten()
        .any(|attribute| {
            is_xmlns_attribute(attribute.key.as_ref()) && attribute.value.as_ref() == namespace
        })
}

fn push_dynamic_declaration(
    declarations: &mut Vec<(String, &'static str)>,
    prefix: String,
    uri: &'static str,
) -> Option<()> {
    if let Some((_, existing_uri)) = declarations
        .iter()
        .find(|(existing_prefix, _)| existing_prefix == &prefix)
    {
        return (*existing_uri == uri).then_some(());
    }
    declarations.push((prefix, uri));
    Some(())
}

fn namespace_ref<'a>(namespace: &'a ResolveResult<'a>) -> Option<&'a [u8]> {
    match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.0),
        _ => None,
    }
}

fn is_xmlns_attribute(name: &[u8]) -> bool {
    name == b"xmlns" || name.starts_with(b"xmlns:")
}

fn canonical_data_composition_name(namespace: Option<&[u8]>, local: &[u8]) -> Option<String> {
    let local = std::str::from_utf8(local).ok()?;
    match namespace {
        Some(DCS_SCHEMA_NS) => Some(local.to_string()),
        Some(DCS_COMMON_NS) => Some(format!("dcscom:{local}")),
        Some(DCS_CORE_NS) => Some(format!("dcscor:{local}")),
        Some(DCS_SETTINGS_NS) => Some(format!("dcsset:{local}")),
        Some(DATA_CORE_NS) => Some(format!("v8:{local}")),
        Some(DATA_UI_NS) => Some(format!("v8ui:{local}")),
        Some(STYLE_NS) => Some(format!("style:{local}")),
        Some(SYS_NS) => Some(format!("sys:{local}")),
        Some(WEB_NS) => Some(format!("web:{local}")),
        Some(WIN_NS) => Some(format!("win:{local}")),
        Some(XSI_NS) => Some(format!("xsi:{local}")),
        Some(XS_NS) => Some(format!("xs:{local}")),
        Some(_) | None => Some(local.to_string()),
    }
}

fn canonical_data_composition_attr_name(namespace: Option<&[u8]>, local: &[u8]) -> Option<String> {
    let local = std::str::from_utf8(local).ok()?;
    match namespace {
        Some(XSI_NS) => Some(format!("xsi:{local}")),
        Some(XS_NS) => Some(format!("xs:{local}")),
        Some(DATA_CORE_NS) => Some(format!("v8:{local}")),
        Some(DATA_UI_NS) => Some(format!("v8ui:{local}")),
        Some(DCS_CORE_NS) => Some(format!("dcscor:{local}")),
        Some(DCS_SETTINGS_NS) => Some(format!("dcsset:{local}")),
        Some(DCS_COMMON_NS) => Some(format!("dcscom:{local}")),
        Some(_) | None => Some(local.to_string()),
    }
}

fn canonical_data_composition_attr_value(
    attr_name: &str,
    value: &str,
    element_namespace: Option<&[u8]>,
) -> String {
    if attr_name != "xsi:type" {
        return value.to_string();
    }
    let suffix = value
        .rsplit_once(':')
        .map(|(_, suffix)| suffix)
        .unwrap_or(value);
    match suffix {
        "LocalStringType" => "v8:LocalStringType".to_string(),
        "Field" => "dcscor:Field".to_string(),
        _ if is_data_core_xsi_type(suffix) => format!("v8:{suffix}"),
        _ if is_dcs_settings_xsi_type(suffix) => format!("dcsset:{suffix}"),
        _ if element_namespace == Some(DCS_CORE_NS) && !value.contains(':') => {
            format!("dcscor:{value}")
        }
        _ if element_namespace == Some(DCS_SETTINGS_NS) && !value.contains(':') => {
            format!("dcsset:{value}")
        }
        _ => value.to_string(),
    }
}

fn is_data_core_xsi_type(value: &str) -> bool {
    matches!(value, "StandardPeriod" | "StandardPeriodVariant")
}

fn is_dcs_settings_xsi_type(value: &str) -> bool {
    matches!(
        value,
        "DataCompositionAttributesPlacement"
            | "DataCompositionChartLegendPlacement"
            | "DataCompositionFixation"
            | "DataCompositionGroupFieldsPlacement"
            | "DataCompositionGroupPlacement"
            | "DataCompositionGroupTemplateType"
            | "DataCompositionGroupUseVariant"
            | "DataCompositionPictureOutputType"
            | "DataCompositionResourcesAutoPosition"
            | "DataCompositionResourcesPlacement"
            | "DataCompositionTextOutputType"
            | "FilterItemComparison"
            | "FilterItemGroup"
            | "GroupItemAuto"
            | "GroupItemField"
            | "OrderItemAuto"
            | "OrderItemField"
            | "SelectedItemAuto"
            | "SelectedItemField"
            | "SelectedItemFolder"
            | "SettingsParameterValue"
            | "StructureItemChart"
            | "StructureItemGroup"
            | "StructureItemNestedObject"
            | "StructureItemTable"
            | "UserFieldCase"
            | "UserFieldExpression"
    )
}
