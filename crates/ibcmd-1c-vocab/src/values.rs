//! Small property values the readers spell: decimal literals, the
//! choice-folders-and-items mode and design-time references.

use std::collections::BTreeMap;

use ibcmd_1c_text::uuid_like_values;

pub fn information_register_decimal_is_valid(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let Some(integer) = parts.next() else {
        return false;
    };
    if integer.is_empty() || !integer.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    match (parts.next(), parts.next()) {
        (None, None) => true,
        (Some(fraction), None) => {
            !fraction.is_empty() && fraction.chars().all(|ch| ch.is_ascii_digit())
        }
        _ => false,
    }
}

pub fn metadata_choice_folders_and_items_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Items"),
        "1" => Some("Folders"),
        "2" => Some("FoldersAndItems"),
        _ => None,
    }
}

pub fn parse_design_time_reference(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    uuid_like_values(text)
        .into_iter()
        .rev()
        .filter_map(|uuid| object_refs.get(&uuid).cloned())
        .next()
}
