//! An extension's adopted form compiled from its `Form.xml`: the form
//! without `<BaseForm>` and `callType` through the native form writer, its
//! event blocks with the interceptors' call types, and the base form after it
//! -- compiled from `<BaseForm>` too, or put back from the base row.
//!
//! It moved from the root's `src/mssql/offline_compile.rs`, whose staging
//! code reads the root's loader, metadata model and exporter; this part reads
//! only the form writer's entry points and crates. The root reaches it as
//! `mssql::offline_compile::adopted_form`.

use std::path::Path;

use anyhow::{Context, Result, bail};

use ibcmd_metadata_source::MetadataSource;

/// Split a direct BaseForm into a complete Form document of its own. Byte
/// spans keep every inherited namespace and source string intact; the base's
/// controls must never enter the ordinary parser for the extension's root.
pub fn split_form_base_xml(xml: &str) -> Result<Option<(String, String)>> {
    use quick_xml::events::Event;
    // The slice reader consumes UTF-8 BOM without adding it to its byte
    // cursor. Normalize the input used for spans, then preserve own's BOM.
    let had_bom = xml.starts_with('\u{feff}');
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let document = ibcmd_xml::XmlReader::from_slice(xml.as_bytes())
        .context("adopted Form.xml is malformed")?;
    if document.root().name().local() != "Form" {
        // without_adoption also folds standalone Events fragments. They have
        // no enclosing Form/BaseForm pair; full body validation stays with
        // the native form writer.
        return Ok(None);
    }
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut depth = 0usize;
    let mut root_open = None::<String>;
    let mut root_name = None::<String>;
    let mut root_version = None::<String>;
    let mut open_base = None::<(usize, usize)>;
    let mut base_span = None::<(usize, usize, usize, usize)>;
    loop {
        let start = usize::try_from(reader.buffer_position())?;
        let event = reader.read_event()?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(node) | Event::Empty(node) => {
                let end = usize::try_from(reader.buffer_position())?;
                let name = node.local_name();
                let version = node
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.local_name().as_ref() == b"version")
                    .map(|a| {
                        a.decode_and_unescape_value(reader.decoder())
                            .map(|v| v.into_owned())
                    })
                    .transpose()?;
                if depth == 0 {
                    root_open = Some(xml[start..end].to_owned());
                    root_name = Some(std::str::from_utf8(node.name().as_ref())?.to_owned());
                    root_version = version;
                } else if name.as_ref() == b"BaseForm" {
                    if depth != 1 || open_base.is_some() || base_span.is_some() {
                        bail!("BaseForm must occur once as a direct Form child");
                    }
                    if let (Some(base), Some(root)) = (version.as_deref(), root_version.as_deref())
                        && base != root
                    {
                        bail!("BaseForm dialect {base} differs from Form dialect {root}");
                    }
                    if empty {
                        base_span = Some((start, end, end, end));
                    } else {
                        open_base = Some((start, end));
                    }
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::End(node) => {
                if node.local_name().as_ref() == b"BaseForm" {
                    let (base_start, inner_start) =
                        open_base.take().context("BaseForm end without start")?;
                    base_span = Some((
                        base_start,
                        usize::try_from(reader.buffer_position())?,
                        inner_start,
                        start,
                    ));
                }
                depth = depth.checked_sub(1).context("Form XML depth underflow")?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let Some((start, end, inner_start, inner_end)) = base_span else {
        return Ok(None);
    };
    let root_open = root_open.context("Form opening tag is missing")?;
    let root_name = root_name.context("Form root name is missing")?;
    let base = format!("{root_open}{}</{root_name}>", &xml[inner_start..inner_end]);
    let mut own = xml.to_owned();
    own.replace_range(start..end, "");
    if had_bom {
        own.insert(0, '\u{feff}');
    }
    Ok(Some((own, base)))
}

/// Compile an adopted form and its inherited base entirely from source XML.
/// The base is a distinct stored form body, not a second batch of root items.
/// A base has no source module here; the extension's BSL belongs only to own.
pub fn compile_adopted_form_body_base_free(
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&dyn MetadataSource>,
    items_root: Option<&Path>,
) -> Result<Option<String>> {
    let xml = std::str::from_utf8(form_xml).context("Form.xml is not UTF-8")?;
    let Some((_, base_xml)) = split_form_base_xml(xml)? else {
        return Ok(None);
    };
    let (own_xml, call_types) = without_adoption(xml)?;
    let own = crate::compile_native_form_body_through(
        own_xml.as_bytes(),
        module_text,
        source,
        items_root,
    )?;
    let own = with_call_type_blocks(&own, &call_types);
    let base =
        crate::compile_native_form_body_through(base_xml.as_bytes(), None, source, items_root)?;
    let trimmed = own.trim_end();
    let head = trimmed
        .strip_suffix(",0,0}")
        .context("compiled adopted form has no ordinary ending")?;
    let base = base.trim().trim_start_matches('\u{feff}');
    let result = format!("{head},1,{base},0}}{}", &own[trimmed.len()..]);
    ibcmd_form_body::parse_form_body_plain(&result)
        .context("source-built adopted form does not parse back")?;
    Ok(Some(result))
}

/// The body of a form an extension adopts, compiled from its `Form.xml`:
/// the form without `<BaseForm>` and `callType` compiles as a configuration's
/// form, its event blocks then take the call types
/// (the root's `mssql_dump::form_extension`: `Before` code 0, `Override` 2,
/// `After` an empty first handler and the handler second, code 1), and the
/// base form body the base row carries is put back after it
/// (`...,1,<base form body>,0` where a form body ends `...,0,0`). An edit of
/// `<BaseForm>` itself is not taken; the load's own check refuses it.
pub fn adopted_form_body(
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&dyn MetadataSource>,
    items_root: Option<&Path>,
    base_body: &[u8],
) -> Result<Vec<u8>> {
    let xml = String::from_utf8(form_xml.to_vec()).context("Form.xml is not UTF-8")?;
    let (own, call_types) = without_adoption(&xml)?;
    let compile = |xml: &str| -> Result<String> {
        let packed = crate::pack_native_form_body_blob_through(
            xml.as_bytes(),
            module_text,
            source,
            items_root,
        )?;
        String::from_utf8(ibcmd_module_blob::inflate_raw(&packed.blob)?)
            .context("the compiled form body is not UTF-8")
    };
    let plain = compile(&own)?;
    let plain = with_call_type_blocks(&plain, &call_types);
    let base_plain = String::from_utf8(ibcmd_module_blob::inflate_raw(base_body)?)
        .context("the base form body is not UTF-8")?;
    let parsed = ibcmd_form_body::parse_form_body_plain(&base_plain)?;
    if parsed
        .trailing
        .get(5)
        .is_some_and(|field| field.trim() == "0")
        && parsed
            .trailing
            .get(6)
            .is_some_and(|field| field.trim() == "0")
    {
        if xml.contains("<BaseForm") {
            bail!("Form.xml adds a BaseForm the stored form does not have");
        }
        // Common forms adopted without a base record keep the ordinary body
        // ending; only their event blocks acquire interceptor codes.
        return ibcmd_module_blob::deflate_raw(plain.as_bytes());
    }
    let base_form = match (
        parsed.trailing.get(5).map(|f| f.trim()),
        parsed.trailing.get(6),
    ) {
        (Some("1"), Some(base)) if base.trim().starts_with('{') => base.trim().to_owned(),
        _ => bail!("the base form body is not an adopted form's"),
    };
    let trimmed = plain.trim_end();
    let Some(head) = trimmed.strip_suffix(",0,0}") else {
        bail!("the compiled form body does not end as a form body does");
    };
    let spliced = format!("{head},1,{base_form},0}}{}", &plain[trimmed.len()..]);
    ibcmd_module_blob::deflate_raw(spliced.as_bytes())
}

/// The stand-in handler an adopted form's event compiles under: one per
/// stored event, its bindings kept aside.
pub const STAND_IN: &str = "ИбкмдПерехватчик";

/// The `(handler, call type)` bindings of one stored event, as the form's
/// `<Event>` lines spell them (the root's `mssql_dump::form_extension`).
type Bindings = Vec<(String, String)>;

/// The form XML without `<BaseForm>`, each event's interceptor lines folded
/// into one line under a stand-in handler, and the stand-ins' bindings.
pub fn without_adoption(
    xml: &str,
) -> Result<(String, std::collections::BTreeMap<String, Bindings>)> {
    ibcmd_xml::XmlReader::from_slice(xml.as_bytes()).context("adopted Form.xml is malformed")?;
    let split = split_form_base_xml(xml)?;
    let has_base = split.is_some();
    let mut reader = quick_xml::Reader::from_str(xml);
    loop {
        match reader.read_event()? {
            quick_xml::events::Event::Start(node) | quick_xml::events::Event::Empty(node) => {
                for attribute in node.attributes() {
                    let attribute = attribute?;
                    if attribute.key.local_name().as_ref() != b"callType" {
                        continue;
                    }
                    if attribute.key.as_ref() != b"callType" {
                        bail!("namespaced callType attributes are unsupported");
                    }
                    let call_type = attribute.decode_and_unescape_value(reader.decoder())?;
                    if !matches!(call_type.as_ref(), "Before" | "After" | "Override") {
                        bail!("unsupported form interceptor callType {call_type:?}");
                    }
                    if !has_base
                        && node.name().local_name().as_ref() == b"Action"
                        && call_type != "Before"
                    {
                        bail!(
                            "a command interceptor without BaseForm is only evidenced for Before"
                        );
                    }
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    let own = split.map_or_else(|| xml.to_owned(), |(own, _)| own);
    let mut stand_ins = std::collections::BTreeMap::new();
    let mut out = String::with_capacity(own.len());
    let mut rest = own.as_str();
    while let Some(open) = rest.find("<Events>") {
        let body_start = open + "<Events>".len();
        let close = body_start
            + rest[body_start..]
                .find("</Events>")
                .context("<Events> is not closed")?;
        out.push_str(&rest[..body_start]);
        out.push_str(&folded_events(&rest[body_start..close], &mut stand_ins)?);
        rest = &rest[close..];
    }
    out.push_str(rest);
    // A differently spelled/prefixed Events container must not bypass the
    // folding pass and silently compile an interceptor as Before.
    let rewritten = ibcmd_xml::XmlReader::from_slice(out.as_bytes())?;
    let mut pending = vec![rewritten.root()];
    while let Some(node) = pending.pop() {
        if node.name().local() == "Event" && node.attributes().iter().any(|attribute|
            matches!(attribute.kind(), ibcmd_xml::AttributeKind::Ordinary(name) if name.local() == "callType")) {
            bail!("unsupported Events XML spelling leaves an uncompiled interceptor");
        }
        pending.extend(node.children().iter().filter_map(|child| match child {
            ibcmd_xml::XmlNode::Element(element) => Some(element),
            _ => None,
        }));
    }
    Ok((out, stand_ins))
}

/// An `<Events>` body with each event's interceptor lines (`callType`)
/// folded into one line under a stand-in; lines without one stay.
fn folded_events(
    body: &str,
    stand_ins: &mut std::collections::BTreeMap<String, Bindings>,
) -> Result<String> {
    // (name, call type, handler) per line, in order.
    let mut lines = Vec::new();
    let mut reader = quick_xml::Reader::from_str(body);
    let mut first_start = None;
    let mut last_end = 0;
    loop {
        let start = usize::try_from(reader.buffer_position())?;
        match reader.read_event()? {
            quick_xml::events::Event::Start(node) if node.name().as_ref() == b"Event" => {
                reader.read_to_end(node.name())?;
            }
            quick_xml::events::Event::Empty(node) if node.name().as_ref() == b"Event" => {}
            quick_xml::events::Event::Eof => break,
            quick_xml::events::Event::Comment(_) => continue,
            quick_xml::events::Event::Text(text) if text.xml_content()?.trim().is_empty() => {
                continue;
            }
            _ => bail!("unsupported content in form Events"),
        }
        let end = usize::try_from(reader.buffer_position())?;
        first_start.get_or_insert(start);
        let event_xml = ibcmd_xml::XmlReader::from_slice(&body.as_bytes()[start..end])
            .context("form event XML is malformed")?;
        let node = event_xml.root();
        let attr = |name: &str| {
            node.attributes()
                .iter()
                .find_map(|attribute| match attribute.kind() {
                    ibcmd_xml::AttributeKind::Ordinary(key) if key.raw() == name => {
                        Some(attribute.value())
                    }
                    _ => None,
                })
        };
        let name = attr("name").context("form event has no name")?.to_owned();
        let call_type = attr("callType").map(str::to_owned);
        if node
            .children()
            .iter()
            .any(|child| matches!(child, ibcmd_xml::XmlNode::Element(_)))
        {
            bail!("form event handler must be text");
        }
        let handler = node
            .children()
            .iter()
            .filter_map(|child| match child {
                ibcmd_xml::XmlNode::Text(text) => Some(text.value()),
                ibcmd_xml::XmlNode::CData(text) => Some(text.value()),
                _ => None,
            })
            .collect::<String>();
        lines.push((name, call_type, handler, body[start..end].to_owned()));
        last_end = end;
    }
    let Some(first_start) = first_start else {
        return Ok(body.to_owned());
    };
    if lines.iter().all(|(_, call_type, _, _)| call_type.is_none()) {
        return Ok(body.to_owned());
    }
    let indent_start = body[..first_start].rfind('\n').map_or(0, |at| at + 1);
    let indent = &body[indent_start..first_start];
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::BTreeMap<String, Vec<(Option<String>, String, String)>> =
        std::collections::BTreeMap::new();
    for (name, call_type, handler, line) in lines {
        if !groups.contains_key(&name) {
            order.push(name.clone());
        }
        groups
            .entry(name)
            .or_default()
            .push((call_type, handler, line));
    }
    let mut folded = Vec::new();
    for name in order {
        let group = &groups[&name];
        if group.iter().all(|(call_type, _, _)| call_type.is_none()) {
            folded.extend(group.iter().map(|(_, _, line)| line.clone()));
            continue;
        }
        let stand_in = format!("{STAND_IN}{}", stand_ins.len());
        stand_ins.insert(
            stand_in.clone(),
            group
                .iter()
                .map(|(call_type, handler, _)| {
                    (
                        handler.clone(),
                        call_type.clone().unwrap_or_else(|| "Before".to_owned()),
                    )
                })
                .collect(),
        );
        folded.push(format!("<Event name=\"{name}\">{stand_in}</Event>"));
    }
    Ok(format!(
        "{}{}{}",
        &body[..first_start],
        folded.join(&format!("\r\n{indent}")),
        &body[last_end..]
    ))
}

/// The configuration-style event blocks of `plain`
/// (`{N,(event,"handler")xN,1,0,(event,0,1)xN}`), each stand-in's event
/// stored as the platform stores an adopted form's: a `Before`/`Override`
/// binding with a handler first (code 0/2), else an empty first handler;
/// every other binding after it with its code (`Before` 0, `After` 1,
/// `Override` 2).
pub fn with_call_type_blocks(
    plain: &str,
    stand_ins: &std::collections::BTreeMap<String, Bindings>,
) -> String {
    let code = |call_type: &str| match call_type {
        "After" => 1,
        "Override" => 2,
        _ => 0,
    };
    let mut out = String::with_capacity(plain.len() + 256);
    let mut copied = 0;
    let mut at = 0;
    while let Some(offset) = plain[at..].find('{') {
        let start = at + offset;
        at = start + 1;
        let Some((fields, end)) = ibcmd_1c_text::brace_list::fields(plain, start) else {
            continue;
        };
        let Ok(count) = fields.first().copied().unwrap_or_default().parse::<usize>() else {
            continue;
        };
        if count == 0 || fields.len() != 3 + 5 * count {
            continue;
        }
        let head = 1 + 2 * count;
        if fields[head] != "1" || fields[head + 1] != "0" {
            continue;
        }
        let events = (0..count)
            .map(|index| (fields[1 + 2 * index], fields[2 + 2 * index]))
            .collect::<Vec<_>>();
        let configuration_style = events.iter().enumerate().all(|(index, (event, _))| {
            let tail = head + 2 + 3 * index;
            fields[tail] == *event && fields[tail + 1] == "0" && fields[tail + 2] == "1"
        });
        if !configuration_style {
            continue;
        }
        let mut first = Vec::new();
        let mut tails = Vec::new();
        for (event, quoted) in &events {
            let Some(bindings) = stand_ins.get(quoted.trim_matches('"')) else {
                first.push(format!("{event},{quoted}"));
                tails.push(format!("{event},0,1"));
                continue;
            };
            let (first_handler, first_code, extras) = match bindings.first() {
                Some((handler, call_type))
                    if !handler.is_empty()
                        && (call_type == "Before" || call_type == "Override") =>
                {
                    (handler.as_str(), code(call_type), &bindings[1..])
                }
                _ => ("", 0, &bindings[..]),
            };
            first.push(format!(
                "{event},\"{}\"",
                first_handler.replace('"', "\"\"")
            ));
            let mut tail = format!("{event},{first_code},{}", 1 + extras.len());
            for (handler, call_type) in extras {
                tail.push_str(&format!(
                    ",\"{}\",{}",
                    handler.replace('"', "\"\""),
                    code(call_type)
                ));
            }
            tails.push(tail);
        }
        out.push_str(&plain[copied..start]);
        out.push_str(&format!(
            "{{{count},{},1,0,{}}}",
            first.join(","),
            tails.join(",")
        ));
        copied = end;
        at = end;
    }
    out.push_str(&plain[copied..]);
    out
}
