//! The adoption an object header records after its comment.

use crate::external::brace;

/// `{3,{1,0,<uuid>},"name",{synonym},"comment",<belonging>,N,
/// (property uuid,state)×N,<extended object uuid>,M,(property uuid,state,
/// extend value)×M}` -- M is `0` but for a property whose value the
/// extension widens (a real extension: a type, `{"#",<TypeDescription>,{"Pattern",…}}`,
/// 15 times). The short `{2,…}` form ends at the extended object.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Adoption {
    /// The object belongs to the configuration it extends (`1`); an own
    /// object of the extension is `0`.
    pub adopted: bool,
    /// Controlled properties in stored order, with their state (`2`
    /// controlled, `3` extended as well).
    pub properties: Vec<(String, u8)>,
    /// The adopted configuration object; the nil uuid when not recorded.
    pub extended_object: String,
    /// Properties the extension widens: `(property uuid, state, extend
    /// value)`; the object's own field then holds the value it checks.
    pub widened: Vec<(String, u8, String)>,
}

pub const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

impl Adoption {
    pub fn state(&self, property: &str) -> Option<u8> {
        self.properties
            .iter()
            .find(|(uuid, _)| uuid == property)
            .map(|(_, state)| *state)
    }
}

/// The adoption of the header list opening at `header[0]` (`{3,…}`).
pub fn parse(header: &str) -> Option<Adoption> {
    let (fields, _) = brace::fields(header, 0)?;
    let long = match fields.first() {
        Some(&"3") => true,
        Some(&"2") => false,
        _ => return None,
    };
    let adopted = match *fields.get(5)? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    // Counts come from the file; each item takes fields, so one beyond the
    // fields is no header (and the arithmetic below cannot overflow).
    let count: usize = fields.get(6)?.parse().ok()?;
    if count > fields.len() {
        return None;
    }
    let mut properties = Vec::with_capacity(count);
    for index in 0..count {
        let uuid = fields.get(7 + 2 * index)?;
        let state = fields.get(8 + 2 * index)?.parse().ok()?;
        properties.push(((*uuid).to_owned(), state));
    }
    let extended_object = (*fields.get(7 + 2 * count)?).to_owned();
    let mut widened = Vec::new();
    let tail = 8 + 2 * count;
    if long {
        let widened_count: usize = fields.get(tail)?.parse().ok()?;
        if widened_count > fields.len() {
            return None;
        }
        for index in 0..widened_count {
            let at = tail + 1 + 3 * index;
            widened.push((
                (*fields.get(at)?).to_owned(),
                fields.get(at + 1)?.parse().ok()?,
                (*fields.get(at + 2)?).to_owned(),
            ));
        }
        if fields.len() != tail + 1 + 3 * widened_count {
            return None;
        }
    } else if fields.len() != tail {
        return None;
    }
    Some(Adoption {
        adopted,
        properties,
        extended_object,
        widened,
    })
}

/// The header list inside a metadata row: the first `{3,{1,0,<uuid>},`
/// (or `{2,…`).
pub fn header_of<'a>(text: &'a str, uuid: &str) -> Option<&'a str> {
    let marker = format!("{{1,0,{uuid}}}");
    let at = text.find(&marker)?;
    let before = text[..at].trim_end().strip_suffix(',')?.trim_end();
    if !(before.ends_with("{3") || before.ends_with("{2")) {
        return None;
    }
    Some(&text[before.len() - 2..])
}

/// The field right after the header list of `uuid` in a row: a widened
/// element's own type field, the type it checks (a real extension, 15 of 15).
pub fn field_after_header<'a>(text: &'a str, uuid: &str) -> Option<&'a str> {
    let header = header_of(text, uuid)?;
    let (_, end) = brace::fields(header, 0)?;
    let rest = header[end..].trim_start().strip_prefix(',')?.trim_start();
    if !rest.starts_with('{') {
        return None;
    }
    let (_, value_end) = brace::fields(rest, 0)?;
    Some(&rest[..value_end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_beyond_the_fields_is_no_adoption() {
        // Counts come from the file: arithmetic on them must not overflow.
        for header in [
            "{3,{1,0,00000000-0000-0000-0000-000000000015},\"Р\",{0},\"\",1,%s,x}",
            "{3,{1,0,00000000-0000-0000-0000-000000000015},\"Р\",{0},\"\",1,0,\
             00000000-0000-0000-0000-000000000001,%s}",
        ] {
            assert!(
                parse(&header.replace("%s", "18446744073709551615")).is_none(),
                "{header}"
            );
        }
    }

    #[test]
    fn reads_the_controlled_properties_and_the_extended_object() {
        let header = "{3,\r\n{1,0,00000000-0000-0000-0000-000000000015},\"Русский\",\r\n{0},\"\",1,2,\
                      9595ddd6-e72c-47ad-a156-672db811628c,2,4b2a2bcf-a845-41ba-a03d-05b09a2c6c11,3,\
                      00000000-0000-0000-0000-000000000001,0},\"ru\"}";
        let adoption = parse(header).unwrap();
        assert!(adoption.adopted);
        assert_eq!(
            adoption.properties,
            vec![
                ("9595ddd6-e72c-47ad-a156-672db811628c".to_owned(), 2),
                ("4b2a2bcf-a845-41ba-a03d-05b09a2c6c11".to_owned(), 3)
            ]
        );
        assert_eq!(
            adoption.extended_object,
            "00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(
            adoption.state("4b2a2bcf-a845-41ba-a03d-05b09a2c6c11"),
            Some(3)
        );
    }

    #[test]
    fn an_own_object_is_not_adopted() {
        let header = "{3,{1,0,00000000-0000-0000-0000-000000000002},\"Имя\",{0},\"\",0,0,\
                      00000000-0000-0000-0000-000000000000,0}";
        let adoption = parse(header).unwrap();
        assert!(!adoption.adopted);
        assert!(adoption.properties.is_empty());
        assert_eq!(adoption.extended_object, NIL_UUID);
    }

    #[test]
    fn reads_a_widened_type() {
        // Fixture `adopted/widened`, `DefinedTypes/Владелец`.
        let header = "{3,\r\n{1,0,e4000000-0000-4000-8000-000000000003},\"Владелец\",\r\n{0},\"\",1,2,\
                      9595ddd6-e72c-47ad-a156-672db811628c,2,b1053250-abe6-11d4-9434-004095e12fc7,3,\
                      b4000000-0000-4000-8000-000000000003,1,b1053250-abe6-11d4-9434-004095e12fc7,3,\r\n\
                      {\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d,\r\n{\"Pattern\",\r\n\
                      {\"#\",d4000010-0000-4000-8000-000000000003}\r\n}\r\n}\r\n},\r\n{\"Pattern\"}";
        let adoption = parse(header).unwrap();
        assert_eq!(
            adoption.extended_object,
            "b4000000-0000-4000-8000-000000000003"
        );
        assert_eq!(adoption.widened.len(), 1);
        let (property, state, value) = &adoption.widened[0];
        assert_eq!(property, "b1053250-abe6-11d4-9434-004095e12fc7");
        assert_eq!(*state, 3);
        assert!(
            value.starts_with("{\"#\",f5c65050-") && value.ends_with('}'),
            "{value}"
        );
        let row = format!("{{1,{{0,x,y,\r\n{header}\r\n}},0}}");
        assert_eq!(
            field_after_header(&row, "e4000000-0000-4000-8000-000000000003"),
            Some("{\"Pattern\"}")
        );
    }

    #[test]
    fn reads_the_short_form_and_finds_it() {
        // `{2,…}` ends at the extended object: no widened list.
        let text = "{1,{0,{2,{1,0,abc},\"Имя\",{0},\"\",1,1,\
                    9595ddd6-e72c-47ad-a156-672db811628c,2,00000000-0000-0000-0000-000000000001},\"ru\"},0}";
        let header = header_of(text, "abc").unwrap();
        assert!(header.starts_with("{2,{1,0,abc}"));
        let adoption = parse(header).unwrap();
        assert!(adoption.adopted);
        assert_eq!(
            adoption.extended_object,
            "00000000-0000-0000-0000-000000000001"
        );
        assert!(adoption.widened.is_empty());
        // A long header cut short is no header.
        assert_eq!(parse("{3,{1,0,abc},\"Имя\",{0},\"\",0,0,x}"), None);
    }

    #[test]
    fn finds_the_header_list_of_a_row() {
        let text = "{1,\r\n{0,\r\n{3,\r\n{1,0,abc},\"Имя\",{0},\"\",0,0,x,0},\"ru\"},0}";
        assert!(
            header_of(text, "abc")
                .unwrap()
                .starts_with("{3,\r\n{1,0,abc}")
        );
    }
}
