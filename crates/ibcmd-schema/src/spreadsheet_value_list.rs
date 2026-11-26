//! The empty ValueList detail record in the public native spreadsheet corpus.
//!
//! `corpus-spreadsheet-facets/CovSheetValueListDetail` stores this exact profile
//! in both 8.3.27.2214 and 8.5.1.1150. Unlike the form choice-list profile,
//! its final member is zero and XML publishes `lastId` as zero.

/// The runtime identity selects the value-list grammar before shape inference.
pub const TYPE_ID: &str = super::FORM_CHOICE_LIST_VALUE_LIST_TYPE_ID;
pub const XML_TYPE: &str = "v8:ValueListType";
pub const VALUE_TYPE_ELEMENT: &str = "valueType";
pub const LAST_ID_ELEMENT: &str = "lastId";
pub const LAST_ID_TYPE: &str = "xs:decimal";
pub const EMPTY_LAST_ID: &str = "0";

pub fn is_namespace_declaration(name: &str) -> bool {
    name == "xmlns" || name.starts_with("xmlns:")
}

pub fn value_element_namespace_is_supported(name: &str, namespace: Option<&str>) -> bool {
    match name {
        "v" | "d" => namespace == Some("http://v8.1c.ru/8.2/data/spreadsheet"),
        "Value" => namespace == Some("http://v8.1c.ru/8.1/data/core"),
        _ => false,
    }
}

/// `None` is another value kind; `Some(false)` is an unmeasured value list.
pub fn classify_fields(fields: &[&str]) -> Option<bool> {
    let [tag, type_id, ..] = fields else {
        return None;
    };
    if !type_id.trim().eq_ignore_ascii_case(TYPE_ID) {
        return None;
    }
    Some(
        tag.trim() == "\"#\""
            && matches!(fields, [_, _, payload] if parse_empty_payload(payload).is_some()),
    )
}

/// The measured empty spreadsheet profile; populated and typed lists refuse.
pub fn parse_empty_payload(raw: &str) -> Option<()> {
    let fields = super::braced_fields_bounded(raw, 8)?;
    let [
        revision,
        item_type,
        reserved_a,
        reserved_b,
        items,
        value_type,
        reserved_c,
        last_id,
    ] = fields.as_slice()
    else {
        return None;
    };
    (revision.trim() == "6"
        && item_type
            .trim()
            .eq_ignore_ascii_case(super::FORM_CHOICE_LIST_VALUE_LIST_ITEM_TYPE_ID)
        && reserved_a.trim() == "0"
        && reserved_b.trim() == "0"
        && super::braced_fields_bounded(items, 1)?.as_slice() == ["0"]
        && super::braced_fields_bounded(value_type, 1)?.as_slice() == ["\"Pattern\""]
        && reserved_c.trim() == "0"
        && last_id.trim() == "0")
        .then_some(())
}

pub fn empty_stored_value() -> String {
    format!(
        "{{\"#\",{TYPE_ID},{{6,{},0,0,{{0}},{{\"Pattern\"}},0,0}}}}",
        super::FORM_CHOICE_LIST_VALUE_LIST_ITEM_TYPE_ID
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_empty_profile_rejects_unclassified_members() {
        let payload = "{6,1e512aab-1b41-4ef6-9375-f0137be9dd91,0,0,{0},{\"Pattern\"},0,0}";
        assert_eq!(parse_empty_payload(payload), Some(()));
        for unknown in [
            payload.replace("{6,", "{7,"),
            payload.replace("1e512aab", "1e512aac"),
            payload.replace(",0,0,{0}", ",1,0,{0}"),
            payload.replace("{0}", "{1,{\"S\",\"item\"}}"),
            payload.replace("{\"Pattern\"}", "{\"Pattern\",\"S\"}"),
            payload.replace("},0,0}", "},1,0}"),
            payload.replace("},0,0}", "},0,1}"),
            payload.replace("},0,0}", "},0,0,0}"),
        ] {
            assert_eq!(parse_empty_payload(&unknown), None, "{unknown}");
        }
    }
}
