use super::*;

const DCS_SCHEMA_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/schema";
const DCS_COMMON_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/common";
const DCS_CORE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/core";
const DCS_SETTINGS_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/settings";
const DCS_AREA_TEMPLATE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/area-template";
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
const DCS_AREA_TEMPLATE_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/area-template";
const ENTERPRISE_URI: &str = "http://v8.1c.ru/8.1/data/enterprise";
const CURRENT_CONFIG_URI: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const SETTINGS_ROOT_UI_NAMESPACES: &str = " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\"";
const DATA_COMPOSITION_SCHEMA_DOCUMENT_PREFIX: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
const DATA_COMPOSITION_SCHEMA_DOCUMENT_SUFFIX: &str = "\r\n</DataCompositionSchema>";

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum DcsTypeResolution {
    KeepId,
    Type { qname: String },
    TypeSet { qname: String },
}

pub(crate) type DcsTypeIndex = BTreeMap<String, DcsTypeResolution>;

pub(crate) fn normalize_data_composition_schema_template_xml(
    inflated: &[u8],
    type_index: &DcsTypeIndex,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<u8>> {
    let xml_start = find_bytes(inflated, b"<?xml")?;
    let text = std::str::from_utf8(&inflated[xml_start..]).ok()?;
    let documents = split_embedded_xml_documents(text);
    let schema_documents = documents
        .iter()
        .copied()
        .filter(|document| {
            document.contains("<SchemaFile") && document.contains("dataCompositionSchema")
        })
        .collect::<Vec<_>>();
    let settings = documents
        .iter()
        .filter(|document| document.contains("<Settings") && document.contains(DCS_SETTINGS_URI))
        .filter_map(|document| {
            canonicalize_data_composition_settings_document(document, object_refs)
        })
        .collect::<Vec<_>>();
    let mut xml = canonicalize_data_composition_schema_documents(&schema_documents, object_refs)?;
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
    let name = tag.split_whitespace().next().unwrap_or("");
    name.strip_prefix("dcscor:").unwrap_or(name)
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

fn canonicalize_data_composition_schema_document(
    document: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let mut writer = DataCompositionXmlWriter::new(object_refs);
    writer.fixed_decl_and_schema_root();
    let root_len = writer.output.len();
    writer.write_document(document, DataCompositionDocumentMode::Schema)?;
    let body = writer.output[root_len..].to_string();
    writer.output.truncate(root_len);
    writer
        .output
        .push_str(&normalize_data_composition_schema_body_indent(&body));
    writer
        .output
        .push_str(DATA_COMPOSITION_SCHEMA_DOCUMENT_SUFFIX);
    Some(writer.output)
}

fn canonicalize_data_composition_schema_documents(
    documents: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let (first, remaining) = documents.split_first()?;
    let mut merged = canonicalize_data_composition_schema_document(first, object_refs)?;
    let mut insertion_offset = direct_settings_variant_insertion_offset(&merged)?;
    for document in remaining {
        let canonical = match data_composition_schema_requires_external_resolution(document)? {
            false => canonicalize_data_composition_schema_document(document, object_refs)?,
            true => {
                if let Some(resolved) =
                    resolve_data_composition_area_template_document(document, object_refs)
                {
                    canonicalize_data_composition_schema_document(&resolved, object_refs)?
                } else if data_composition_inline_area_template_document_is_self_contained(
                    document,
                    object_refs,
                )? {
                    canonicalize_data_composition_schema_document(document, object_refs)?
                } else {
                    continue;
                }
            }
        };
        let body = canonical
            .strip_prefix(DATA_COMPOSITION_SCHEMA_DOCUMENT_PREFIX)?
            .strip_suffix(DATA_COMPOSITION_SCHEMA_DOCUMENT_SUFFIX)?;
        if body.is_empty() {
            continue;
        }
        merged.insert_str(insertion_offset, body);
        insertion_offset += body.len();
    }
    Some(merged)
}

#[derive(Debug)]
struct DcsAreaAppearance {
    body: String,
    source_indent: String,
    namespace_declarations: String,
    has_unsupported_color_ref: bool,
}

#[derive(Debug)]
struct DcsAreaReplacement {
    start: usize,
    end: usize,
    index: usize,
    target_indent: String,
}

#[derive(Debug)]
struct DcsAreaStorageFrame {
    namespace: Option<Vec<u8>>,
    local: Vec<u8>,
    start: usize,
    content_start: usize,
    element_children: usize,
    app_index_children: usize,
    last_element_child_is_app_index: bool,
    app_index_text: Option<String>,
    app_index_text_events: usize,
    is_area_template: bool,
    is_data_ui_color_value: bool,
    has_unsupported_color_ref: bool,
    namespace_declarations: String,
}

fn resolve_data_composition_area_template_document(
    document: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let mut reader = NsReader::from_str(document);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<DcsAreaStorageFrame>::new();
    let mut appearances = Vec::<DcsAreaAppearance>::new();
    let mut replacements = Vec::<DcsAreaReplacement>::new();
    let mut schema_children = 0usize;
    let mut saw_schema = false;
    let mut schema_has_unsupported_color_ref = false;

    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                let local = local.as_ref();
                let event_len = event.as_ref().len().checked_add(2)?;
                let content_start = usize::try_from(reader.buffer_position()).ok()?;
                let start = content_start.checked_sub(event_len)?;

                if stack.is_empty() {
                    if namespace.is_some() || local != b"SchemaFile" {
                        return None;
                    }
                } else {
                    let parent = stack.last_mut()?;
                    parent.element_children = parent.element_children.checked_add(1)?;
                    if parent.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                        && parent.local.as_slice() == b"tableCell"
                    {
                        let is_app_index =
                            namespace == Some(DCS_AREA_TEMPLATE_NS) && local == b"appIndex";
                        parent.last_element_child_is_app_index = is_app_index;
                        if is_app_index {
                            parent.app_index_children = parent.app_index_children.checked_add(1)?;
                        }
                    }
                }

                let is_direct_schema_file_child = stack.len() == 1;
                let is_inner_schema = is_direct_schema_file_child
                    && namespace == Some(DCS_SCHEMA_NS)
                    && local == b"dataCompositionSchema";
                let is_outer_appearance = is_direct_schema_file_child
                    && namespace == Some(DCS_AREA_TEMPLATE_NS)
                    && local == b"appearance";
                if is_direct_schema_file_child {
                    schema_children = schema_children.checked_add(1)?;
                    if is_inner_schema {
                        if saw_schema {
                            return None;
                        }
                        saw_schema = true;
                    } else if !is_outer_appearance || !saw_schema {
                        return None;
                    }
                }

                let is_app_index = namespace == Some(DCS_AREA_TEMPLATE_NS) && local == b"appIndex";
                if is_app_index {
                    if !stack.iter().any(|frame| {
                        frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                            && frame.local.as_slice() == b"dataCompositionSchema"
                    }) || !stack.last().is_some_and(|parent| {
                        parent.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                            && parent.local.as_slice() == b"tableCell"
                    }) || !stack.iter().any(|frame| frame.is_area_template)
                        || event.attributes().with_checks(false).count() != 0
                    {
                        return None;
                    }
                }

                let is_area_template = data_composition_xsi_type_is(
                    &reader,
                    &event,
                    DCS_AREA_TEMPLATE_NS,
                    b"AreaTemplate",
                )?;
                let is_data_ui_color_value = namespace == Some(DCS_CORE_NS)
                    && local == b"value"
                    && data_composition_xsi_type_is(&reader, &event, DATA_UI_NS, b"Color")?;
                stack.push(DcsAreaStorageFrame {
                    namespace: namespace.map(<[u8]>::to_vec),
                    local: local.to_vec(),
                    start,
                    content_start,
                    element_children: 0,
                    app_index_children: 0,
                    last_element_child_is_app_index: false,
                    app_index_text: None,
                    app_index_text_events: 0,
                    is_area_template,
                    is_data_ui_color_value,
                    has_unsupported_color_ref: false,
                    namespace_declarations: if is_outer_appearance {
                        data_composition_namespace_declarations(&reader, &event)?
                    } else {
                        String::new()
                    },
                });
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                let local = local.as_ref();
                let Some(parent) = stack.last_mut() else {
                    return None;
                };
                parent.element_children = parent.element_children.checked_add(1)?;
                if parent.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                    && parent.local.as_slice() == b"tableCell"
                {
                    parent.last_element_child_is_app_index = false;
                }
                if stack.len() == 1
                    || namespace == Some(DCS_AREA_TEMPLATE_NS) && local == b"appIndex"
                {
                    return None;
                }
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                let local = local.as_ref();
                let position = usize::try_from(reader.buffer_position()).ok()?;
                let end_tag_len = event.name().as_ref().len().checked_add(3)?;
                let content_end = position.checked_sub(end_tag_len)?;
                let frame = stack.pop()?;
                if frame.namespace.as_deref() != namespace || frame.local.as_slice() != local {
                    return None;
                }

                if namespace == Some(DCS_AREA_TEMPLATE_NS) && local == b"appIndex" {
                    if frame.element_children != 0 || frame.app_index_text_events != 1 {
                        return None;
                    }
                    let text = frame.app_index_text?;
                    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
                        return None;
                    }
                    replacements.push(DcsAreaReplacement {
                        start: frame.start,
                        end: position,
                        index: text.parse().ok()?,
                        target_indent: data_composition_line_indent(document, frame.start)?
                            .to_string(),
                    });
                }

                if namespace == Some(DCS_AREA_TEMPLATE_NS)
                    && local == b"tableCell"
                    && (frame.app_index_children > 1
                        || frame.app_index_children == 1 && !frame.last_element_child_is_app_index)
                {
                    return None;
                }

                if stack.len() == 1
                    && namespace == Some(DCS_AREA_TEMPLATE_NS)
                    && local == b"appearance"
                {
                    if frame.element_children == 0 || content_end < frame.content_start {
                        return None;
                    }
                    appearances.push(DcsAreaAppearance {
                        body: document[frame.content_start..content_end].to_string(),
                        source_indent: data_composition_line_indent(document, frame.start)?
                            .to_string(),
                        namespace_declarations: frame.namespace_declarations,
                        has_unsupported_color_ref: frame.has_unsupported_color_ref,
                    });
                }

                if frame.has_unsupported_color_ref
                    && let Some(parent) = stack.last_mut()
                {
                    parent.has_unsupported_color_ref = true;
                }
            }
            Event::Text(event) => {
                let text = std::str::from_utf8(event.as_ref()).ok()?;
                if let Some(frame) = stack.last_mut()
                    && frame.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                    && frame.local.as_slice() == b"appIndex"
                {
                    frame.app_index_text_events = frame.app_index_text_events.checked_add(1)?;
                    frame.app_index_text = Some(text.to_string());
                }
                if stack
                    .last()
                    .is_some_and(|frame| frame.is_data_ui_color_value)
                    && serialized_data_composition_color_ref_uuid(text).is_some()
                    && data_composition_style_item_name(text, object_refs).is_none()
                {
                    if stack.iter().any(|frame| {
                        frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                            && frame.local.as_slice() == b"dataCompositionSchema"
                    }) {
                        schema_has_unsupported_color_ref = true;
                    }
                    if let Some(appearance) = stack.iter_mut().find(|frame| {
                        frame.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                            && frame.local.as_slice() == b"appearance"
                            && frame.start < content_position(&reader)
                    }) {
                        appearance.has_unsupported_color_ref = true;
                    }
                }
            }
            Event::CData(_) | Event::GeneralRef(_) => {
                if stack.last().is_some_and(|frame| {
                    frame.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                        && frame.local.as_slice() == b"appIndex"
                }) {
                    return None;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let referenced_indices = replacements
        .iter()
        .map(|replacement| replacement.index)
        .collect::<BTreeSet<_>>();
    if !stack.is_empty()
        || !saw_schema
        || schema_children != appearances.len().checked_add(1)?
        || appearances.is_empty()
        || replacements.is_empty()
        || schema_has_unsupported_color_ref
        || referenced_indices.len() != appearances.len()
        || !(0..appearances.len()).all(|index| referenced_indices.contains(&index))
        || replacements.iter().any(|replacement| {
            replacement.index >= appearances.len()
                || appearances[replacement.index].has_unsupported_color_ref
        })
    {
        return None;
    }

    let mut resolved = document.to_string();
    replacements.sort_by_key(|replacement| replacement.start);
    if replacements
        .windows(2)
        .any(|pair| pair[0].end > pair[1].start)
    {
        return None;
    }
    for replacement in replacements.iter().rev() {
        let appearance = appearances.get(replacement.index)?;
        let body = reindent_data_composition_fragment(
            &appearance.body,
            &appearance.source_indent,
            &replacement.target_indent,
        )?;
        let replacement_xml = format!(
            "<dcsat:appearance xmlns:dcsat=\"{DCS_AREA_TEMPLATE_URI}\"{}>{body}</dcsat:appearance>",
            appearance.namespace_declarations
        );
        resolved.replace_range(replacement.start..replacement.end, &replacement_xml);
    }
    Some(resolved)
}

fn content_position(reader: &NsReader<&[u8]>) -> usize {
    usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX)
}

fn data_composition_xsi_type_is(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    expected_namespace: &[u8],
    expected_local: &[u8],
) -> Option<bool> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (namespace, local) = reader.resolve_attribute(attribute.key);
        if namespace_ref(&namespace) != Some(XSI_NS) || local.as_ref() != b"type" {
            continue;
        }
        let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
        let (namespace, local) = reader.resolve(quick_xml::name::QName(value.as_bytes()), false);
        return Some(
            namespace_ref(&namespace) == Some(expected_namespace)
                && local.as_ref() == expected_local,
        );
    }
    Some(false)
}

fn data_composition_namespace_declarations(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
) -> Option<String> {
    let mut declarations = String::new();
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let name = attribute.key.as_ref();
        if !is_xmlns_attribute(name) || name == b"xmlns:dcsat" {
            continue;
        }
        declarations.push(' ');
        declarations.push_str(std::str::from_utf8(name).ok()?);
        declarations.push_str("=\"");
        declarations.push_str(&escape_xml_text(
            &attribute.decode_and_unescape_value(reader.decoder()).ok()?,
        ));
        declarations.push('"');
    }
    Some(declarations)
}

fn data_composition_line_indent(text: &str, position: usize) -> Option<&str> {
    let line_start = text[..position].rfind('\n').map_or(0, |offset| offset + 1);
    let indent = &text[line_start..position];
    indent
        .bytes()
        .all(|byte| matches!(byte, b'\t' | b' '))
        .then_some(indent)
}

fn reindent_data_composition_fragment(
    fragment: &str,
    source_indent: &str,
    target_indent: &str,
) -> Option<String> {
    let mut output = String::with_capacity(fragment.len());
    for (index, line) in fragment.split_inclusive('\n').enumerate() {
        if index == 0 {
            if !matches!(line, "\n" | "\r\n") {
                return None;
            }
            output.push_str(line);
            continue;
        }
        if let Some(rest) = line.strip_prefix(source_indent) {
            output.push_str(target_indent);
            output.push_str(rest);
        } else if matches!(line, "\n" | "\r\n") {
            output.push_str(line);
        } else {
            return None;
        }
    }
    Some(output)
}

fn serialized_data_composition_color_ref_uuid(text: &str) -> Option<String> {
    let uuid = text.trim().strip_prefix("0:")?;
    if uuid.len() != 36 {
        return None;
    }
    let canonical = uuid::Uuid::parse_str(uuid).ok()?.hyphenated().to_string();
    canonical.eq_ignore_ascii_case(uuid).then_some(canonical)
}

fn data_composition_style_item_name(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let uuid = serialized_data_composition_color_ref_uuid(text)?;
    let reference = object_refs.get(&uuid).or_else(|| {
        let source_uuid = text.trim().strip_prefix("0:")?;
        object_refs.get(source_uuid)
    })?;
    let name = reference.strip_prefix("StyleItem.")?;
    (!name.is_empty()).then(|| name.to_string())
}

#[derive(Debug)]
struct DcsInlineAreaFrame {
    namespace: Option<Vec<u8>>,
    local: Vec<u8>,
    is_data_ui_color: bool,
}

pub(super) fn data_composition_inline_area_template_document_is_self_contained(
    document: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<bool> {
    let mut reader = NsReader::from_str(document);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<DcsInlineAreaFrame>::new();
    let mut schema_children = 0usize;
    let mut saw_schema = false;
    let mut saw_area_template = false;

    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                let local = local.as_ref();
                if stack.is_empty() {
                    if namespace.is_some() || local != b"SchemaFile" {
                        return Some(false);
                    }
                } else if stack.len() == 1 {
                    schema_children = schema_children.checked_add(1)?;
                    if namespace != Some(DCS_SCHEMA_NS)
                        || local != b"dataCompositionSchema"
                        || saw_schema
                    {
                        return Some(false);
                    }
                    saw_schema = true;
                }
                if namespace == Some(DCS_AREA_TEMPLATE_NS) && local == b"appIndex"
                    || stack.len() == 1
                        && namespace == Some(DCS_AREA_TEMPLATE_NS)
                        && local == b"appearance"
                {
                    return Some(false);
                }
                saw_area_template |= namespace == Some(DCS_AREA_TEMPLATE_NS)
                    || data_composition_xsi_type_uses_namespace(
                        &reader,
                        &event,
                        DCS_AREA_TEMPLATE_NS,
                    )?;
                let is_data_ui_color =
                    data_composition_xsi_type_is(&reader, &event, DATA_UI_NS, b"Color")?;
                stack.push(DcsInlineAreaFrame {
                    namespace: namespace.map(<[u8]>::to_vec),
                    local: local.to_vec(),
                    is_data_ui_color,
                });
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                let local = local.as_ref();
                if stack.is_empty()
                    || stack.len() == 1
                    || namespace == Some(DCS_AREA_TEMPLATE_NS) && local == b"appIndex"
                {
                    return Some(false);
                }
                saw_area_template |= namespace == Some(DCS_AREA_TEMPLATE_NS)
                    || data_composition_xsi_type_uses_namespace(
                        &reader,
                        &event,
                        DCS_AREA_TEMPLATE_NS,
                    )?;
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let frame = stack.pop()?;
                if frame.namespace.as_deref() != namespace_ref(&namespace)
                    || frame.local.as_slice() != local.as_ref()
                {
                    return None;
                }
            }
            Event::Text(event) => {
                let text = std::str::from_utf8(event.as_ref()).ok()?;
                let is_serialized_color_ref =
                    serialized_data_composition_color_ref_uuid(text).is_some();
                let is_resolvable_style_value = stack.last().is_some_and(|frame| {
                    frame.is_data_ui_color
                        && frame.namespace.as_deref() == Some(DCS_CORE_NS)
                        && frame.local.as_slice() == b"value"
                        && data_composition_style_item_name(text, object_refs).is_some()
                });
                if stack.is_empty() && !is_xml_whitespace(text)
                    || is_serialized_color_ref
                        && stack.iter().any(|frame| frame.is_data_ui_color)
                        && !is_resolvable_style_value
                {
                    return Some(false);
                }
            }
            Event::CData(_) | Event::GeneralRef(_)
                if stack.iter().any(|frame| frame.is_data_ui_color) =>
            {
                return Some(false);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    Some(stack.is_empty() && saw_schema && schema_children == 1 && saw_area_template)
}

fn data_composition_xsi_type_uses_namespace(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    expected_namespace: &[u8],
) -> Option<bool> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (namespace, local) = reader.resolve_attribute(attribute.key);
        if namespace_ref(&namespace) != Some(XSI_NS) || local.as_ref() != b"type" {
            continue;
        }
        let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
        let (namespace, _) = reader.resolve(quick_xml::name::QName(value.as_bytes()), false);
        return Some(namespace_ref(&namespace) == Some(expected_namespace));
    }
    Some(false)
}

fn data_composition_schema_requires_external_resolution(document: &str) -> Option<bool> {
    let mut reader = NsReader::from_str(document);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                let local = local.as_ref();
                if stack.last().is_some_and(|(namespace, local)| {
                    namespace.is_none() && local.as_slice() == b"SchemaFile"
                }) && (namespace != Some(DCS_SCHEMA_NS) || local != b"dataCompositionSchema")
                    || namespace == Some(DCS_AREA_TEMPLATE_NS)
                    || event_declares_namespace(&event, DCS_AREA_TEMPLATE_NS)
                {
                    return Some(true);
                }
                stack.push((namespace.map(<[u8]>::to_vec), local.to_vec()));
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_ref(&namespace);
                if stack.last().is_some_and(|(namespace, local)| {
                    namespace.is_none() && local.as_slice() == b"SchemaFile"
                }) && (namespace != Some(DCS_SCHEMA_NS)
                    || local.as_ref() != b"dataCompositionSchema")
                    || namespace == Some(DCS_AREA_TEMPLATE_NS)
                    || event_declares_namespace(&event, DCS_AREA_TEMPLATE_NS)
                {
                    return Some(true);
                }
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
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
    stack.is_empty().then_some(false)
}

fn direct_settings_variant_insertion_offset(xml: &str) -> Option<usize> {
    let offsets = direct_settings_variant_offsets(xml)?;
    let mut offset = match offsets.first() {
        Some(offsets) => offsets.opening,
        None => xml
            .strip_suffix(DATA_COMPOSITION_SCHEMA_DOCUMENT_SUFFIX)
            .map(str::len)?,
    };
    while offset > 0 && matches!(xml.as_bytes()[offset - 1], b'\r' | b'\n' | b'\t' | b' ') {
        offset -= 1;
    }
    Some(offset)
}

fn canonicalize_data_composition_settings_document(
    document: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let mut writer = DataCompositionXmlWriter::new(object_refs);
    writer.write_document(document, DataCompositionDocumentMode::Settings)?;
    let settings = writer
        .output
        .trim_start_matches(['\r', '\n', '\t'])
        .to_string();
    Some(indent_data_composition_settings(&settings))
}

fn insert_data_composition_settings(xml: &mut String, settings: &[String]) -> Option<()> {
    let offsets = direct_settings_variant_offsets(xml)?;
    if offsets.len() != settings.len() {
        return None;
    }
    for (offsets, settings_block) in offsets.into_iter().zip(settings.iter()).rev() {
        xml.insert_str(offsets.closing, settings_block);
    }
    Some(())
}

struct DirectSettingsVariantOffsets {
    opening: usize,
    closing: usize,
}

fn direct_settings_variant_offsets(xml: &str) -> Option<Vec<DirectSettingsVariantOffsets>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>, usize)>::new();
    let mut offsets = Vec::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let event_len = event.as_ref().len().checked_add(2)?;
                let position = usize::try_from(reader.buffer_position()).ok()?;
                stack.push((
                    namespace_ref(&namespace).map(<[u8]>::to_vec),
                    local.as_ref().to_vec(),
                    position.checked_sub(event_len)?,
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
                    && stack.first().is_some_and(|(namespace, local, _)| {
                        namespace.as_deref() == Some(DCS_SCHEMA_NS)
                            && local.as_slice() == b"DataCompositionSchema"
                    })
                    && namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                    && local.as_ref() == b"settingsVariant"
                {
                    let end_tag_len = event.name().as_ref().len() + 3;
                    let position = usize::try_from(reader.buffer_position()).ok()?;
                    offsets.push(DirectSettingsVariantOffsets {
                        opening: stack.last()?.2,
                        closing: position.checked_sub(end_tag_len)?,
                    });
                }
                let (open_namespace, open_local, _) = stack.pop()?;
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DataCompositionDocumentMode {
    Schema,
    Settings,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DcsDynamicNamespace {
    prefix: String,
    uri: String,
}

#[derive(Debug)]
struct DcsElementFrame {
    namespace: Option<Vec<u8>>,
    local: Vec<u8>,
    rendered_name: String,
    xsi_type_local: Option<String>,
    dynamic_namespaces: Vec<DcsDynamicNamespace>,
    is_data_ui_color_value: bool,
    output_namespace_offset: usize,
}

#[derive(Debug, Clone)]
struct DcsExpandedQName {
    namespace: Option<Vec<u8>>,
    local: String,
}

#[derive(Debug)]
struct DcsRenderedQName {
    value: String,
    declaration: Option<(String, String)>,
}

struct DcsWrittenStart {
    rendered_name: String,
    dynamic_namespaces: Vec<DcsDynamicNamespace>,
    output_namespace_offset: usize,
}

struct DataCompositionXmlWriter<'a> {
    output: String,
    skip_depth: usize,
    element_stack: Vec<DcsElementFrame>,
    object_refs: &'a BTreeMap<String, String>,
}

impl<'a> DataCompositionXmlWriter<'a> {
    fn new(object_refs: &'a BTreeMap<String, String>) -> Self {
        Self {
            output: String::new(),
            skip_depth: 0,
            element_stack: Vec::new(),
            object_refs,
        }
    }

    fn fixed_decl_and_schema_root(&mut self) {
        self.output
            .push_str(DATA_COMPOSITION_SCHEMA_DOCUMENT_PREFIX);
    }

    fn write_document(&mut self, document: &str, mode: DataCompositionDocumentMode) -> Option<()> {
        let mut reader = NsReader::from_str(document);
        reader.config_mut().trim_text(false);
        let mut schema_depth =
            matches!(mode, DataCompositionDocumentMode::Schema).then_some(0usize);
        let mut saw_schema_root = false;
        loop {
            match reader.read_event().ok()? {
                Event::Start(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    let local = local.as_ref();
                    if let Some(depth) = schema_depth.as_mut() {
                        if *depth == 0 {
                            if namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                                && local == b"dataCompositionSchema"
                            {
                                if saw_schema_root {
                                    return None;
                                }
                                saw_schema_root = true;
                                *depth = 1;
                            }
                            continue;
                        }
                        *depth = depth.checked_add(1)?;
                    }
                    if self.skip_depth == 0 {
                        let written_start = self.write_start_tag(
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
                            written_start,
                        )?);
                    }
                }
                Event::Empty(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    if schema_depth == Some(0) {
                        if namespace_ref(&namespace) == Some(DCS_SCHEMA_NS)
                            && local.as_ref() == b"dataCompositionSchema"
                        {
                            if saw_schema_root {
                                return None;
                            }
                            saw_schema_root = true;
                        }
                        continue;
                    }
                    if self.skip_depth == 0 {
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
                    if let Some(depth) = schema_depth.as_mut() {
                        if *depth == 0 {
                            continue;
                        }
                        if *depth == 1 {
                            if namespace_ref(&namespace) != Some(DCS_SCHEMA_NS)
                                || local != b"dataCompositionSchema"
                                || !self.element_stack.is_empty()
                            {
                                return None;
                            }
                            *depth = 0;
                            continue;
                        }
                    }
                    let frame = self.element_stack.pop()?;
                    if frame.namespace.as_deref() != namespace_ref(&namespace)
                        || frame.local.as_slice() != local
                    {
                        return None;
                    }
                    self.output.push_str("</");
                    self.output.push_str(&frame.rendered_name);
                    self.output.push('>');
                    if let Some(depth) = schema_depth.as_mut() {
                        *depth = depth.checked_sub(1)?;
                    }
                }
                Event::Text(event) => {
                    if self.skip_depth == 0 && schema_depth.is_none_or(|depth| depth > 0) {
                        self.write_text(&reader, &event, &mode)?;
                    }
                }
                Event::CData(event) => {
                    if self.skip_depth == 0 && schema_depth.is_none_or(|depth| depth > 0) {
                        self.output.push_str("<![CDATA[");
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push_str("]]>");
                    }
                }
                Event::Comment(event) => {
                    if self.skip_depth == 0 && schema_depth.is_none_or(|depth| depth > 0) {
                        self.output.push_str("<!--");
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push_str("-->");
                    }
                }
                Event::GeneralRef(event) => {
                    if self.skip_depth == 0 && schema_depth.is_none_or(|depth| depth > 0) {
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
        if schema_depth.is_some() && (!saw_schema_root || schema_depth != Some(0)) {
            return None;
        }
        self.element_stack.is_empty().then_some(())
    }

    fn write_start_tag(
        &mut self,
        reader: &NsReader<&[u8]>,
        event: &quick_xml::events::BytesStart<'_>,
        namespace: Option<&[u8]>,
        local: &[u8],
        empty: bool,
        mode: &DataCompositionDocumentMode,
    ) -> Option<DcsWrittenStart> {
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
        let mut rendered_attributes = Vec::<(String, String)>::new();
        let mut dynamic_namespaces = Vec::<DcsDynamicNamespace>::new();
        let is_data_ui_picture_value = namespace == Some(DCS_CORE_NS)
            && local == b"value"
            && data_composition_xsi_type_is(reader, event, DATA_UI_NS, b"Picture")?;
        if namespace == Some(DATA_CORE_NS)
            && matches!(local, b"Type" | b"TypeSet")
            && event_declares_namespace(event, CURRENT_CONFIG_NS)
        {
            self.push_dynamic_namespace(
                &mut dynamic_namespaces,
                self.current_config_prefix(),
                CURRENT_CONFIG_URI.to_string(),
            )?;
        }
        for attribute in event.attributes().with_checks(false) {
            let attribute = attribute.ok()?;
            if is_xmlns_attribute(attribute.key.as_ref()) {
                continue;
            }
            let (attr_namespace, attr_local) = reader.resolve_attribute(attribute.key);
            let rendered_attr_name = self.render_data_composition_node_name(
                attribute.key.as_ref(),
                namespace_ref(&attr_namespace),
                attr_local.as_ref(),
                true,
                *mode,
                &dynamic_namespaces,
            )?;
            if let Some((prefix, uri)) = rendered_attr_name.declaration {
                self.push_dynamic_namespace(&mut dynamic_namespaces, prefix, uri)?;
            }
            let attr_name = rendered_attr_name.value;
            let value = attribute
                .decode_and_unescape_value(reader.decoder())
                .ok()?
                .into_owned();
            let value = if attr_name == "ref" && is_data_ui_picture_value {
                canonical_data_composition_picture_ref(reader, &value).unwrap_or(value)
            } else {
                value
            };
            let rendered = if attr_name == "xsi:type" {
                let rendered = self.render_xsi_type(
                    reader,
                    &value,
                    namespace,
                    local,
                    *mode,
                    &dynamic_namespaces,
                );
                if value.contains(':') {
                    Some(rendered?)
                } else {
                    rendered
                }
            } else {
                None
            };
            let value = if let Some(rendered) = rendered {
                if let Some((prefix, uri)) = rendered.declaration {
                    self.push_dynamic_namespace(&mut dynamic_namespaces, prefix, uri)?;
                }
                rendered.value
            } else {
                canonical_data_composition_attr_value(&attr_name, &value, namespace)
            };
            rendered_attributes.push((attr_name, value));
        }
        let name = if is_settings_root {
            "dcsset:settings".to_string()
        } else {
            let rendered_name = self.render_data_composition_node_name(
                event.name().as_ref(),
                namespace,
                local,
                false,
                *mode,
                &dynamic_namespaces,
            )?;
            if let Some((prefix, uri)) = rendered_name.declaration {
                self.push_dynamic_namespace(&mut dynamic_namespaces, prefix, uri)?;
            }
            rendered_name.value
        };
        self.output.push('<');
        self.output.push_str(&name);
        let output_namespace_offset = self.output.len();
        if is_settings_root || is_inline_settings_root {
            self.output.push_str(SETTINGS_ROOT_UI_NAMESPACES);
        }
        for namespace in &dynamic_namespaces {
            self.output.push_str(" xmlns:");
            self.output.push_str(&namespace.prefix);
            self.output.push_str("=\"");
            self.output.push_str(&namespace.uri);
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
        Some(DcsWrittenStart {
            rendered_name: name,
            dynamic_namespaces,
            output_namespace_offset,
        })
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
                    && !self
                        .element_stack
                        .last()?
                        .dynamic_namespaces
                        .iter()
                        .any(|namespace| &namespace.prefix == prefix)
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
        if self
            .element_stack
            .last()
            .is_some_and(|frame| frame.is_data_ui_color_value)
        {
            let value = text.trim();
            let value_start = text.find(value)?;
            let in_area_template = self.element_stack.iter().any(|frame| {
                frame.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                    && frame.local.as_slice() == b"appearance"
            });
            let resolved_style_name = data_composition_style_item_name(text, self.object_refs);
            let qualified_area_value = (in_area_template && value.contains(':'))
                .then(|| resolve_data_composition_qname(reader, value))
                .flatten()
                .and_then(|expanded| {
                    let namespace = expanded.namespace?;
                    Some((namespace, expanded.local))
                });
            if let Some((namespace, local)) = resolved_style_name
                .map(|name| (STYLE_NS.to_vec(), name))
                .or(qualified_area_value)
            {
                let prefix = in_area_template.then(|| self.scope_prefix(8));
                if let Some(prefix) = &prefix {
                    let output_namespace_offset =
                        self.element_stack.last()?.output_namespace_offset;
                    let namespace = std::str::from_utf8(&namespace).ok()?;
                    self.output.insert_str(
                        output_namespace_offset,
                        &format!(" xmlns:{prefix}=\"{}\"", escape_xml_text(namespace)),
                    );
                }
                self.output.push_str(&text[..value_start]);
                self.output.push_str(prefix.as_deref().unwrap_or("style"));
                self.output.push(':');
                self.output.push_str(&escape_xml_text(&local));
                self.output.push_str(&text[value_start + value.len()..]);
                return Some(());
            }
        }
        self.output.push_str(text);
        Some(())
    }

    fn render_data_composition_node_name(
        &self,
        lexical_name: &[u8],
        namespace: Option<&[u8]>,
        local: &[u8],
        is_attribute: bool,
        mode: DataCompositionDocumentMode,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        if namespace.is_none() && lexical_name.contains(&b':') {
            return None;
        }
        let canonical = if is_attribute {
            canonical_data_composition_attr_name(namespace, local)
        } else {
            canonical_data_composition_name(namespace, local)
        };
        if let Some(value) = canonical {
            return Some(DcsRenderedQName {
                value,
                declaration: None,
            });
        }
        self.render_dynamic_qname(
            lexical_name,
            DcsExpandedQName {
                namespace: Some(namespace?.to_vec()),
                local: std::str::from_utf8(local).ok()?.to_string(),
            },
            mode,
            local_namespaces,
        )
    }

    fn render_dynamic_qname(
        &self,
        lexical_name: &[u8],
        expanded: DcsExpandedQName,
        mode: DataCompositionDocumentMode,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        let uri = std::str::from_utf8(expanded.namespace.as_deref()?).ok()?;
        if let Some(prefix) = self.output_prefix_for_namespace(uri, local_namespaces) {
            return Some(DcsRenderedQName {
                value: format!("{prefix}:{}", expanded.local),
                declaration: None,
            });
        }
        let lexical_name = std::str::from_utf8(lexical_name).ok()?;
        let (input_prefix, lexical_local) = lexical_name.split_once(':')?;
        if lexical_local != expanded.local {
            return None;
        }
        let output_prefix = data_composition_output_scope_prefix(input_prefix, mode)?;
        if let Some(existing_uri) =
            self.dynamic_namespace_uri_for_prefix(&output_prefix, local_namespaces)
        {
            if existing_uri != uri {
                return None;
            }
            return Some(DcsRenderedQName {
                value: format!("{output_prefix}:{}", expanded.local),
                declaration: None,
            });
        }
        Some(DcsRenderedQName {
            value: format!("{output_prefix}:{}", expanded.local),
            declaration: Some((output_prefix, uri.to_string())),
        })
    }

    fn output_prefix_for_namespace(
        &self,
        uri: &str,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<String> {
        local_namespaces
            .iter()
            .rev()
            .chain(
                self.element_stack
                    .iter()
                    .rev()
                    .flat_map(|frame| frame.dynamic_namespaces.iter().rev()),
            )
            .find(|namespace| namespace.uri == uri)
            .map(|namespace| namespace.prefix.clone())
            .or_else(|| {
                globally_declared_data_composition_prefix(uri.as_bytes()).map(str::to_string)
            })
    }

    fn dynamic_namespace_uri_for_prefix<'b>(
        &'b self,
        prefix: &str,
        local_namespaces: &'b [DcsDynamicNamespace],
    ) -> Option<&'b str> {
        local_namespaces
            .iter()
            .rev()
            .chain(
                self.element_stack
                    .iter()
                    .rev()
                    .flat_map(|frame| frame.dynamic_namespaces.iter().rev()),
            )
            .find(|namespace| namespace.prefix == prefix)
            .map(|namespace| namespace.uri.as_str())
    }

    fn push_dynamic_namespace(
        &self,
        namespaces: &mut Vec<DcsDynamicNamespace>,
        prefix: String,
        uri: String,
    ) -> Option<()> {
        if reserved_data_composition_namespace_uri(&prefix)
            .is_some_and(|reserved_uri| reserved_uri.as_bytes() != uri.as_bytes())
        {
            return None;
        }
        if self
            .element_stack
            .iter()
            .rev()
            .flat_map(|frame| frame.dynamic_namespaces.iter().rev())
            .find(|namespace| namespace.prefix == prefix)
            .is_some_and(|namespace| namespace.uri != uri)
        {
            return None;
        }
        if let Some(existing) = namespaces
            .iter()
            .find(|namespace| namespace.prefix == prefix)
        {
            return (existing.uri == uri).then_some(());
        }
        namespaces.push(DcsDynamicNamespace { prefix, uri });
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
        mode: DataCompositionDocumentMode,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        if value.contains(':') {
            let expanded = resolve_data_composition_qname(reader, value)?;
            if let Some(rendered) = self.render_expanded_qname(expanded.clone(), element_local) {
                return Some(rendered);
            }
            return self.render_dynamic_qname(value.as_bytes(), expanded, mode, local_namespaces);
        }
        let namespace = if element_namespace == Some(DCS_AREA_TEMPLATE_NS) {
            Some(DCS_AREA_TEMPLATE_NS)
        } else if is_data_core_xsi_type(value) {
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
        if namespace == Some(DCS_AREA_TEMPLATE_NS) {
            let prefix = "dcsat".to_string();
            let declaration = (!self.element_stack.iter().any(|frame| {
                frame
                    .dynamic_namespaces
                    .iter()
                    .any(|namespace| namespace.prefix == prefix)
            }))
            .then(|| (prefix.clone(), DCS_AREA_TEMPLATE_URI.to_string()));
            return Some(DcsRenderedQName {
                value: format!("{prefix}:{}", expanded.local),
                declaration,
            });
        }
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
            Some(CURRENT_CONFIG_NS) => {
                (self.current_config_prefix(), CURRENT_CONFIG_URI.to_string())
            }
            Some(ENTERPRISE_NS) => (
                self.enterprise_prefix(element_local),
                ENTERPRISE_URI.to_string(),
            ),
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
    written_start: DcsWrittenStart,
) -> Option<DcsElementFrame> {
    let mut xsi_type_local = None;
    let mut is_data_ui_color_value = false;
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (attr_namespace, attr_local) = reader.resolve_attribute(attribute.key);
        if namespace_ref(&attr_namespace) == Some(XSI_NS) && attr_local.as_ref() == b"type" {
            let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
            let expanded = resolve_data_composition_qname(reader, &value)?;
            is_data_ui_color_value = namespace == Some(DCS_CORE_NS)
                && local == b"value"
                && expanded.namespace.as_deref() == Some(DATA_UI_NS)
                && expanded.local == "Color";
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
        rendered_name: written_start.rendered_name,
        xsi_type_local,
        dynamic_namespaces: written_start.dynamic_namespaces,
        is_data_ui_color_value,
        output_namespace_offset: written_start.output_namespace_offset,
    })
}

fn resolve_data_composition_qname(
    reader: &NsReader<&[u8]>,
    value: &str,
) -> Option<DcsExpandedQName> {
    let (namespace, local) = reader.resolve(quick_xml::name::QName(value.as_bytes()), false);
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.0.to_vec()),
        ResolveResult::Unbound if value.contains(':') => return None,
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

fn data_composition_output_scope_prefix(
    input_prefix: &str,
    mode: DataCompositionDocumentMode,
) -> Option<String> {
    if mode == DataCompositionDocumentMode::Settings
        && let Some(number) = input_prefix
            .strip_prefix('d')
            .and_then(|value| value.strip_suffix("p1"))
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        && let Ok(number) = number.parse::<usize>()
        && input_prefix == format!("d{number}p1")
    {
        return Some(format!("d{}p1", number.checked_add(2)?));
    }
    Some(input_prefix.to_string())
}

fn globally_declared_data_composition_prefix(namespace: &[u8]) -> Option<&'static str> {
    match namespace {
        DCS_COMMON_NS => Some("dcscom"),
        DCS_CORE_NS => Some("dcscor"),
        DCS_SETTINGS_NS => Some("dcsset"),
        DATA_CORE_NS => Some("v8"),
        DATA_UI_NS => Some("v8ui"),
        XSI_NS => Some("xsi"),
        XS_NS => Some("xs"),
        _ => None,
    }
}

fn reserved_data_composition_namespace_uri(prefix: &str) -> Option<&'static str> {
    let namespace = match prefix {
        "dcscom" => DCS_COMMON_NS,
        "dcscor" => DCS_CORE_NS,
        "dcsset" => DCS_SETTINGS_NS,
        "dcsat" => DCS_AREA_TEMPLATE_NS,
        "v8" => DATA_CORE_NS,
        "v8ui" => DATA_UI_NS,
        "style" => STYLE_NS,
        "sys" => SYS_NS,
        "web" => WEB_NS,
        "win" => WIN_NS,
        "xsi" => XSI_NS,
        "xs" => XS_NS,
        _ => return None,
    };
    std::str::from_utf8(namespace).ok()
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
        Some(DCS_AREA_TEMPLATE_NS) => Some(format!("dcsat:{local}")),
        Some(DATA_CORE_NS) => Some(format!("v8:{local}")),
        Some(DATA_UI_NS) => Some(format!("v8ui:{local}")),
        Some(STYLE_NS) => Some(format!("style:{local}")),
        Some(SYS_NS) => Some(format!("sys:{local}")),
        Some(WEB_NS) => Some(format!("web:{local}")),
        Some(WIN_NS) => Some(format!("win:{local}")),
        Some(XSI_NS) => Some(format!("xsi:{local}")),
        Some(XS_NS) => Some(format!("xs:{local}")),
        Some(_) => None,
        None => Some(local.to_string()),
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
        Some(DCS_AREA_TEMPLATE_NS) => Some(format!("dcsat:{local}")),
        Some(_) => None,
        None => Some(local.to_string()),
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
        _ if element_namespace == Some(DCS_AREA_TEMPLATE_NS) && !value.contains(':') => {
            format!("dcsat:{value}")
        }
        _ => value.to_string(),
    }
}

fn canonical_data_composition_picture_ref(reader: &NsReader<&[u8]>, value: &str) -> Option<String> {
    if !value.contains(':') {
        return None;
    }
    let expanded = resolve_data_composition_qname(reader, value)?;
    (expanded.namespace.as_deref() == Some(DATA_UI_NS) && !expanded.local.is_empty())
        .then(|| format!("v8ui:{}", expanded.local))
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

#[cfg(test)]
mod tests {
    use super::*;

    const TYPE_ID: &str = "11111111-1111-1111-1111-111111111111";

    fn rewritten_type(xml: &str) -> String {
        let mut xml = xml.to_string();
        let type_index = BTreeMap::from([(
            TYPE_ID.to_string(),
            DcsTypeResolution::Type {
                qname: "cfg:Catalog.Test".to_string(),
            },
        )]);
        rewrite_data_composition_type_ids(&mut xml, &type_index);
        xml
    }

    #[test]
    fn prefixed_core_item_uses_dataset_object_current_config_prefix() {
        let xml = format!(
            "<dcscor:item xsi:type=\"DataSetObject\"><v8:TypeId>{TYPE_ID}</v8:TypeId></dcscor:item>"
        );

        assert_eq!(
            rewritten_type(&xml),
            format!(
                "<dcscor:item xsi:type=\"DataSetObject\"><v8:Type xmlns:d6p1=\"{CURRENT_CONFIG_URI}\">d6p1:Catalog.Test</v8:Type></dcscor:item>"
            )
        );
    }

    #[test]
    fn parameter_and_calculated_field_keep_d4_current_config_prefix() {
        for context in ["parameter", "calculatedField"] {
            let xml = format!("<{context}><v8:TypeId>{TYPE_ID}</v8:TypeId></{context}>");

            assert_eq!(
                rewritten_type(&xml),
                format!(
                    "<{context}><v8:Type xmlns:d4p1=\"{CURRENT_CONFIG_URI}\">d4p1:Catalog.Test</v8:Type></{context}>"
                )
            );
        }
    }

    #[test]
    fn unprefixed_dataset_object_item_keeps_d6_current_config_prefix() {
        let xml =
            format!("<item xsi:type=\"DataSetObject\"><v8:TypeId>{TYPE_ID}</v8:TypeId></item>");

        assert_eq!(
            rewritten_type(&xml),
            format!(
                "<item xsi:type=\"DataSetObject\"><v8:Type xmlns:d6p1=\"{CURRENT_CONFIG_URI}\">d6p1:Catalog.Test</v8:Type></item>"
            )
        );
    }

    #[test]
    fn foreign_prefixed_contexts_keep_default_current_config_prefix() {
        for (xml, expected) in [
            (
                format!("<foreign:parameter><v8:TypeId>{TYPE_ID}</v8:TypeId></foreign:parameter>"),
                format!(
                    "<foreign:parameter><v8:Type xmlns:d5p1=\"{CURRENT_CONFIG_URI}\">d5p1:Catalog.Test</v8:Type></foreign:parameter>"
                ),
            ),
            (
                format!(
                    "<foreign:item xsi:type=\"DataSetObject\"><v8:TypeId>{TYPE_ID}</v8:TypeId></foreign:item>"
                ),
                format!(
                    "<foreign:item xsi:type=\"DataSetObject\"><v8:Type xmlns:d5p1=\"{CURRENT_CONFIG_URI}\">d5p1:Catalog.Test</v8:Type></foreign:item>"
                ),
            ),
        ] {
            assert_eq!(rewritten_type(&xml), expected);
        }
    }
}
