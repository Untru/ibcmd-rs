//! What the form reader and the 8.5 down-conversion read of an adopted
//! form's body: its event block, every handler of every event with its
//! call type code, and the slot of its base form record; and the adoption
//! model the form writer spells ([`FormAdoption`], [`FormBaseForm`]), with
//! the constructor of an adopted form that keeps no base form record
//! ([`adoption_without_base_form`]). From `src/mssql_dump/form_extension.rs`,
//! which keeps the constructors that read the root (`form_adoption`,
//! `FormBaseForm::from_document`).

use std::collections::BTreeMap;

use anyhow::Result;
use ibcmd_1c_text::{
    parse_1c_quoted_string_with_len, scan_1c_braced_value, split_1c_braced_fields,
};

use crate::ParsedFormBodyBlob;
use crate::form_body::{
    FormAutoCommandBar, FormBodyEvent, FormChildItem, FormCommand, form_event_name_from_identifier,
};
use crate::layout_8_5_1::group_event_name_8_5_1;

/// One handler of one event, as stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormEventBinding {
    pub event: String,
    pub handler: String,
    pub code: u8,
}

pub const CALL_TYPES: [&str; 3] = ["Before", "After", "Override"];

/// A complete quoted handler token. Bytes after its closing quote are
/// unsupported data, rather than part of the event record we can discard.
fn parse_event_handler(field: &str) -> Option<String> {
    let token = field.trim();
    let (handler, consumed) = parse_1c_quoted_string_with_len(token)?;
    (consumed == token.len()).then_some(handler)
}

/// The bindings of the event block `field`, in stored order: an event's first
/// handler (when it has one), then its further handlers.
pub fn parse_form_event_block(field: &str) -> Option<Vec<FormEventBinding>> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let count: usize = fields.first()?.trim().parse().ok()?;
    if count == 0 || count > fields.len() || fields.len() < 3 + 5 * count {
        return None;
    }
    let mut first = Vec::with_capacity(count);
    for index in 0..count {
        let event = fields.get(1 + 2 * index)?.trim();
        let handler = parse_event_handler(fields.get(2 + 2 * index)?)?;
        first.push((event, handler));
    }
    let mut at = 1 + 2 * count;
    if fields.get(at)?.trim() != "1" || fields.get(at + 1)?.trim() != "0" {
        return None;
    }
    at += 2;
    let mut out = Vec::new();
    for (event, handler) in first {
        if fields.get(at)?.trim() != event {
            return None;
        }
        let code: u8 = fields.get(at + 1)?.trim().parse().ok()?;
        let handlers: usize = fields.get(at + 2)?.trim().parse().ok()?;
        if handlers == 0 || usize::from(code) >= CALL_TYPES.len() {
            return None;
        }
        at += 3;
        if !handler.is_empty() {
            out.push(FormEventBinding {
                event: event.to_owned(),
                handler,
                code,
            });
        }
        for _ in 1..handlers {
            let extra = parse_event_handler(fields.get(at)?)?;
            let code: u8 = fields.get(at + 1)?.trim().parse().ok()?;
            if usize::from(code) >= CALL_TYPES.len() {
                return None;
            }
            at += 2;
            out.push(FormEventBinding {
                event: event.to_owned(),
                handler: extra,
                code,
            });
        }
    }
    (at == fields.len()).then_some(out)
}

/// The trailing section of an adopted form that holds its base form record.
pub fn adopted_base_record_slot(body: &ParsedFormBodyBlob) -> Option<usize> {
    let flag = body.trailing.get(5)?.trim();
    let base = body.trailing.get(6)?.trim();
    if flag != "1" || !base.starts_with('{') {
        return None;
    }
    Some(6)
}

/// Call types of the handlers of a form body: by event and handler, and by
/// handler alone for an event whose name the block does not spell. A key
/// bound under two call types is left out (nothing to choose from).
#[derive(Debug, Default)]
pub struct EventCallTypes {
    by_event: BTreeMap<(String, String), Option<&'static str>>,
    by_handler: BTreeMap<String, Option<&'static str>>,
}

impl EventCallTypes {
    fn insert(&mut self, event: String, handler: String, call_type: &'static str) {
        fn merge<K: Ord>(
            map: &mut BTreeMap<K, Option<&'static str>>,
            key: K,
            call_type: &'static str,
        ) {
            map.entry(key)
                .and_modify(|known| {
                    if *known != Some(call_type) {
                        *known = None;
                    }
                })
                .or_insert(Some(call_type));
        }
        merge(&mut self.by_handler, handler.clone(), call_type);
        merge(&mut self.by_event, (event, handler), call_type);
    }

    /// The call type of `handler` bound to the event `event` (its XML name).
    pub fn get(&self, event: &str, handler: &str) -> Option<&'static str> {
        match self.by_event.get(&(event.to_owned(), handler.to_owned())) {
            Some(known) => *known,
            None => self.by_handler.get(handler).copied().flatten(),
        }
    }

    #[cfg(test)]
    fn handler(handler: &str, call_type: &'static str) -> Self {
        let mut map = Self::default();
        map.by_handler.insert(handler.to_owned(), Some(call_type));
        map
    }
}

pub fn adoption_event_name(identifier: &str) -> Option<&'static str> {
    form_event_name_from_identifier(identifier).or_else(|| group_event_name_8_5_1(identifier))
}
/// Call types over every event block of the form body text. One procedure
/// may intercept two events under two call types (fixture
/// `adopted/form_events_shared`: `Before` on OnOpen, `After` on BeforeClose),
/// so the event is part of the key.
pub fn form_event_call_types(text: &str) -> EventCallTypes {
    let mut found = EventCallTypes::default();
    let bytes = text.as_bytes();
    for (start, byte) in bytes.iter().enumerate() {
        if *byte != b'{' {
            continue;
        }
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        let Some(bindings) = parse_form_event_block(&text[start..end]) else {
            continue;
        };
        for binding in bindings {
            let identifier = binding.event.trim().trim_matches('"').to_owned();
            let event = adoption_event_name(&identifier).map_or(identifier, str::to_owned);
            found.insert(
                event,
                binding.handler,
                CALL_TYPES[usize::from(binding.code)],
            );
        }
    }
    found
}

/// What the writer spells for an adopted form beyond its body, read once by
/// the root's `form_extension::form_adoption` and handed to the writer of the
/// form.
#[derive(Debug)]
pub struct FormAdoption {
    /// The call type of every event handler of the form.
    pub call_types: EventCallTypes,
    /// The call type of every command handler: `Before` on the adopted form
    /// (the six commands of an extension's common form in the БСП 8.3.27
    /// ServiceDesk all say so; where the command record keeps a code for an
    /// interceptor that would say otherwise is not on record), none on its
    /// base form, whose commands are not on record either way.
    pub command_call_type: Option<&'static str>,
    /// The base form the extension adopted, written.
    pub base_form: Option<FormBaseForm>,
}

impl FormAdoption {
    /// The stored call type of an event restored by a dialect-specific writer.
    pub fn event_call_type(&self, event: &str, handler: &str) -> Option<&'static str> {
        self.call_types.get(event, handler)
    }
    /// Hands every event handler of the form's model its call type, and every
    /// command handler the form's: the writer then spells `callType` where
    /// the model says so.
    pub fn mark(
        &self,
        events: &mut [FormBodyEvent],
        auto_command_bar: Option<&mut FormAutoCommandBar>,
        child_items: &mut [FormChildItem],
        commands: &mut [FormCommand],
    ) {
        self.mark_events(events);
        if let Some(command_bar) = auto_command_bar {
            self.mark_items(&mut command_bar.child_items);
        }
        self.mark_items(child_items);
        for command in commands {
            command.call_type = self.command_call_type;
        }
    }

    fn mark_events(&self, events: &mut [FormBodyEvent]) {
        for event in events {
            event.call_type = self.call_types.get(&event.name, &event.handler);
        }
    }

    /// Every item's own events and its tooltip's, down the item tree (context
    /// menus, command bars and additions are items of it).
    fn mark_items(&self, items: &mut [FormChildItem]) {
        for item in items {
            self.mark_events(&mut item.events);
            if let Some(tooltip) = item.extended_tooltip.as_mut() {
                self.mark_events(&mut tooltip.events);
            }
            self.mark_items(&mut item.child_items);
        }
    }
}

/// The base form an adopted form carries, written as a document of its own:
/// what `<BaseForm version="…">` encloses, one level deeper.
#[derive(Debug)]
pub struct FormBaseForm {
    /// The dialect of the document, `2.20` or `2.21`.
    pub version: &'static str,
    /// The children of the document's root as written: CRLF lines at the
    /// root's indentation.
    pub children: String,
}

/// The adoption facts of an adopted form that keeps no base form record
/// (ServiceDesk's adopted `CommonForm.СвязанныеДокументы`, fixture
/// `native-evidence/extension-form-interceptors`): the call types its event
/// blocks store, `Before` on its commands, no `<BaseForm>`. An event block
/// whose head is recognized but whose tail is not is refused rather than
/// read as `Before`.
pub fn adoption_without_base_form(layout: &str) -> Result<FormAdoption> {
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
        if adoption_event_name(identifier.trim().trim_matches('"')).is_some()
            && fields
                .first()
                .is_some_and(|count| count.trim().parse::<usize>().is_ok())
            && parse_form_event_block(block).is_none()
        {
            anyhow::bail!("unsupported or malformed adopted form event block for {identifier}");
        }
    }
    Ok(FormAdoption {
        call_types: form_event_call_types(layout),
        // `Before`.
        command_call_type: Some(CALL_TYPES[0]),
        base_form: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_body::{FormExtendedTooltip, format_form_body_close_xml};

    fn event(name: &str, handler: &str) -> FormBodyEvent {
        FormBodyEvent {
            name: name.to_owned(),
            handler: handler.to_owned(),
            call_type: None,
        }
    }

    #[test]
    fn marks_the_call_type_of_a_known_handler_down_the_item_tree() {
        let adoption = FormAdoption {
            call_types: EventCallTypes::handler("Перед", "Before"),
            command_call_type: Some("Before"),
            base_form: None,
        };
        let mut events = vec![event("OnOpen", "Перед"), event("x", "y")];
        let mut item = FormChildItem::default();
        item.events.push(event("OnChange", "Перед"));
        let nested = FormChildItem {
            extended_tooltip: Some(FormExtendedTooltip {
                events: vec![event("Click", "Перед")],
                ..FormExtendedTooltip::default()
            }),
            ..FormChildItem::default()
        };
        item.child_items.push(nested);
        let mut items = vec![item];
        adoption.mark(&mut events, None, &mut items, &mut []);
        assert_eq!(events[0].call_type, Some("Before"));
        assert_eq!(events[1].call_type, None);
        assert_eq!(items[0].events[0].call_type, Some("Before"));
        let tooltip = items[0].child_items[0].extended_tooltip.as_ref().unwrap();
        assert_eq!(tooltip.events[0].call_type, Some("Before"));
    }

    #[test]
    fn the_base_form_closes_the_document_one_level_deeper() {
        let base = FormBaseForm {
            version: "2.20",
            children: "\t<Events>\r\n\
                       \t\t<Event name=\"OnOpen\" callType=\"Before\">П</Event>\r\n\
                       \t</Events>\r\n"
                .to_owned(),
        };
        assert_eq!(
            format_form_body_close_xml(Some(&base)),
            "\t<BaseForm version=\"2.20\">\r\n\
             \t\t<Events>\r\n\
             \t\t\t<Event name=\"OnOpen\" callType=\"Before\">П</Event>\r\n\
             \t\t</Events>\r\n\
             \t</BaseForm>\r\n\
             </Form>"
        );
        let empty = FormBaseForm {
            version: "2.21",
            children: String::new(),
        };
        assert_eq!(
            format_form_body_close_xml(Some(&empty)),
            "\t<BaseForm version=\"2.21\"/>\r\n</Form>"
        );
        assert_eq!(format_form_body_close_xml(None), "</Form>");
    }

    #[test]
    fn a_count_beyond_the_fields_is_no_event_block() {
        assert!(parse_form_event_block("{18446744073709551615,e,\"h\",1,0,e,0,1}").is_none());
    }

    #[test]
    fn first_handler_token_must_be_consumed_completely() {
        assert!(parse_form_event_block(r#"{1,e,"Handler"UNMODELED,1,0,e,0,1}"#).is_none());
    }

    #[test]
    fn additional_handler_token_must_be_consumed_completely() {
        assert!(
            parse_form_event_block(r#"{1,e,"",1,0,e,0,2,"AfterHandler"UNMODELED,1}"#).is_none()
        );
    }

    #[test]
    fn complete_handler_tokens_keep_whitespace_and_doubled_quotes() {
        let bindings =
            parse_form_event_block(r#"{1,e, "First""Handler" ,1,0,e,0,2, "After""Handler" ,1}"#)
                .unwrap();
        assert_eq!(bindings[0].handler, "First\"Handler");
        assert_eq!(bindings[1].handler, "After\"Handler");
    }

    #[test]
    fn reads_every_handler_of_an_interceptor_block() {
        let block = "{3,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"Перед\",52dbb775-1631-4fd5-8c55-1615b5881dac,\
                     \"Вместо\",9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"\",1,0,\
                     3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,1,52dbb775-1631-4fd5-8c55-1615b5881dac,2,1,\
                     9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,2,\"После\",1}";
        let bindings = parse_form_event_block(block).unwrap();
        let spelled = bindings
            .iter()
            .map(|b| (b.handler.as_str(), CALL_TYPES[usize::from(b.code)]))
            .collect::<Vec<_>>();
        assert_eq!(
            spelled,
            vec![
                ("Перед", "Before"),
                ("Вместо", "Override"),
                ("После", "After")
            ]
        );
        assert_eq!(bindings[2].event, "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b");
    }

    #[test]
    fn a_configuration_block_reads_one_handler_per_event() {
        let block = "{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"ПриСозданииНаСервере\",1,0,\
                     9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,1}";
        assert_eq!(parse_form_event_block(block).unwrap().len(), 1);
        assert!(parse_form_event_block("{1,x,\"h\",1,0,y,0,1}").is_none());
    }
}
