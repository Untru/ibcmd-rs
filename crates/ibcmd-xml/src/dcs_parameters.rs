//! Script-variant spelling of built-in data-composition parameter names.

use crate::dcs_schema::DcsInnerSchemaError;
use quick_xml::{NsReader, events::Event, name::ResolveResult};

const CORE: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/core";
const SCHEMA: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/schema";
const SETTINGS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/settings";
const AREA: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/area-template";

/// Applies the English script variant to standard appearance, output and input
/// parameter keys. User data parameters and all surrounding bytes are retained.
/// The caller determines the script variant from the configuration root record.
pub fn with_english_parameter_names(source: &[u8]) -> Result<Vec<u8>, DcsInnerSchemaError> {
    // quick-xml positions are relative to its input after BOM detection.
    // Strip it explicitly so replacement spans use the original byte offsets.
    let body = source.strip_prefix(b"\xef\xbb\xbf").unwrap_or(source);
    let offset = source.len() - body.len();
    let mut reader = NsReader::from_reader(body);
    let mut stack = Vec::<(Vec<u8>, Vec<u8>)>::new();
    let mut candidate = None;
    let mut replacements = Vec::new();
    let mut events = 0;
    loop {
        let before = offset + reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|error| DcsInnerSchemaError::Malformed(error.to_string()))?;
        events += 1;
        if events > 1_000_000 {
            return Err(malformed("DCS parameter-name event limit"));
        }
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                if candidate.is_some() {
                    return Err(malformed("DCS standard parameter contains an element"));
                }
                let (namespace, local) = reader.resolve_element(element.name());
                let namespace = match namespace {
                    ResolveResult::Bound(namespace) => namespace.as_ref().to_vec(),
                    ResolveResult::Unbound => Vec::new(),
                    ResolveResult::Unknown(_) => {
                        return Err(malformed("unbound DCS parameter-name namespace"));
                    }
                };
                let is_key = namespace == CORE
                    && local.as_ref() == b"parameter"
                    && matches!(stack.as_slice(), [.., container, item] if item.0 == CORE && item.1 == b"item" && standard_container(container));
                // Empty elements have no name text to translate.
                if empty {
                    continue;
                }
                if is_key {
                    candidate = Some(offset + reader.buffer_position() as usize);
                }
                stack.push((namespace, local.as_ref().to_vec()));
                if stack.len() > 256 {
                    return Err(malformed("DCS parameter-name depth limit"));
                }
            }
            Event::End(_) => {
                if let Some(start) = candidate.take() {
                    let raw = std::str::from_utf8(&source[start..before])
                        .map_err(|_| malformed("DCS parameter name is not UTF-8"))?;
                    let name = quick_xml::escape::unescape(raw)
                        .map_err(|error| DcsInnerSchemaError::Malformed(error.to_string()))?;
                    if let Some(english) = ibcmd_schema::dcs_parameter_names::english_name(&name) {
                        replacements.push((start, before, english));
                    }
                }
                stack
                    .pop()
                    .ok_or_else(|| malformed("unexpected DCS parameter-name closing element"))?;
            }
            Event::Eof => break,
            Event::Text(_) | Event::GeneralRef(_) => {}
            _ if candidate.is_some() => {
                return Err(malformed("unsupported content in DCS standard parameter"));
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(malformed("unclosed DCS parameter-name element"));
    }
    let mut output = Vec::with_capacity(source.len());
    let mut copied = 0;
    for (start, end, name) in replacements {
        output.extend_from_slice(&source[copied..start]);
        output.extend_from_slice(name.as_bytes());
        copied = end;
    }
    output.extend_from_slice(&source[copied..]);
    Ok(output)
}

fn standard_container(container: &(Vec<u8>, Vec<u8>)) -> bool {
    let (namespace, local) = container;
    (matches!(namespace.as_slice(), SCHEMA | SETTINGS | AREA) && local == b"appearance")
        || (namespace == SCHEMA && local == b"inputParameters")
        || (namespace == SETTINGS && local == b"outputParameters")
}

fn malformed(message: &str) -> DcsInnerSchemaError {
    DcsInnerSchemaError::Malformed(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_keys_change_without_touching_values_or_user_names() {
        let source = format!(
            "<schema xmlns=\"{}\" xmlns:c=\"{}\" xmlns:s=\"{}\"><appearance><c:item><c:parameter>ЦветФона</c:parameter><c:value>ЦветФона</c:value></c:item></appearance><inputParameters><c:item><c:parameter>ФормаВыбора</c:parameter></c:item></inputParameters><s:outputParameters><c:item><c:parameter>Заголовок</c:parameter></c:item></s:outputParameters><s:dataParameters><c:item><c:parameter>Заголовок</c:parameter></c:item></s:dataParameters><appearance><c:item><c:parameter>CustomName</c:parameter></c:item></appearance></schema>",
            String::from_utf8_lossy(SCHEMA),
            String::from_utf8_lossy(CORE),
            String::from_utf8_lossy(SETTINGS)
        );
        let expected = source
            .replacen(
                "<c:parameter>ЦветФона</c:parameter>",
                "<c:parameter>BackColor</c:parameter>",
                1,
            )
            .replacen(
                "<c:parameter>ФормаВыбора</c:parameter>",
                "<c:parameter>ChoiceForm</c:parameter>",
                1,
            )
            .replacen(
                "<c:parameter>Заголовок</c:parameter>",
                "<c:parameter>Title</c:parameter>",
                1,
            );
        assert_eq!(
            with_english_parameter_names(source.as_bytes()).unwrap(),
            expected.as_bytes()
        );
    }

    #[test]
    fn bom_offsets_preserve_the_complete_utf8_document() {
        let source = format!(
            "\u{feff}<appearance xmlns=\"{}\" xmlns:c=\"{}\"><c:item><c:parameter>Заголовок</c:parameter></c:item></appearance>",
            String::from_utf8_lossy(SCHEMA),
            String::from_utf8_lossy(CORE)
        );
        let expected = source.replace("Заголовок", "Title");
        assert_eq!(
            with_english_parameter_names(source.as_bytes()).unwrap(),
            expected.as_bytes()
        );
    }

    #[test]
    fn namespace_identity_controls_standard_context() {
        let source = format!(
            "<s:root xmlns:s=\"{}\" xmlns=\"{}\" xmlns:other=\"urn:other\"><s:appearance><item><parameter>ЦветФона</parameter></item></s:appearance><other:appearance><item><parameter>ЦветФона</parameter></item></other:appearance><s:appearance><other:item><parameter>ЦветФона</parameter></other:item></s:appearance><s:appearance><item><other:parameter>ЦветФона</other:parameter></item></s:appearance></s:root>",
            String::from_utf8_lossy(SCHEMA),
            String::from_utf8_lossy(CORE)
        );
        let expected = source.replacen(
            "<parameter>ЦветФона</parameter>",
            "<parameter>BackColor</parameter>",
            1,
        );
        assert_eq!(
            with_english_parameter_names(source.as_bytes()).unwrap(),
            expected.as_bytes()
        );
    }

    #[test]
    fn malformed_or_opaque_standard_key_is_rejected() {
        for value in ["<nested/>", "<![CDATA[ЦветФона]]>"] {
            let source = format!(
                "<appearance xmlns=\"{}\" xmlns:c=\"{}\"><c:item><c:parameter>{value}</c:parameter></c:item></appearance>",
                String::from_utf8_lossy(SCHEMA),
                String::from_utf8_lossy(CORE)
            );
            assert!(with_english_parameter_names(source.as_bytes()).is_err());
        }
    }
}
