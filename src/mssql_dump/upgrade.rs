//! Metadata records stored in older record versions, upgraded the way the
//! platform upgrades them when it loads a configuration.
//!
//! A configuration saved by an older platform keeps its records in the
//! object-record versions of that platform; 8.3.27 reads them and, on save,
//! writes the current version. The readers of this crate know the current
//! versions (and some older ones); a rule here rewrites a record from an older
//! version into one they read, member by member, only where the platform's own
//! upgrade is evidenced:
//!
//! - pairs of a corpus as stored and the same configuration re-serialized by
//!   8.3.27 (`_onecdec/reserialize.py`: its native XML loaded back and saved),
//!   compared record by record (`_onecdec/upgrade_rules.py`);
//! - fixtures derived from a platform-built configuration by the inverse
//!   rewrite, which the platform itself loads and dumps to the XML of the
//!   original (`_onecdec/make_upgrade_fixtures.py`).
//!
//! A record in a version no rule names is left as stored; its reader refuses
//! it as before (fail closed).

use std::ops::Range;

use super::metadata::{MetadataTextRow, parse_metadata_object_code};

/// Upgrade every row whose kind (as the configuration root states it) has a
/// rule for the version the row is stored in.
pub(super) fn upgrade_metadata_text_rows(rows: &mut [MetadataTextRow]) {
    for row in rows {
        let Some(kind) = row.kind.as_deref() else {
            continue;
        };
        if let Some(text) = upgrade_metadata_record(kind, &row.text) {
            row.object_code = parse_metadata_object_code(&text);
            row.text = text;
        }
    }
}

/// The record rewritten into a version the readers know, or `None` when no
/// rule applies (current versions, unknown versions, other kinds).
pub(super) fn upgrade_metadata_record(kind: &str, text: &str) -> Option<String> {
    match kind {
        "ExchangePlan" => upgrade_exchange_plan(text),
        "CommonPicture" => upgrade_common_picture(text),
        "Subsystem" => retag_owner(text, "20", 8, "21"),
        _ => None,
    }
}

/// The owner record `{<from>,…}` of `members` members retagged `<to>`, all
/// members kept.
///
/// Subsystem `{20,…}` -> `{21,…}` (8 members each): re-serialized by 8.3.27,
/// the subsystems of a real configuration stored as 20 and as 21 came back
/// alike -- `{22,…}` with one member `0` appended, nothing else changed (2 and
/// 4 records); the reader reads 21 as it is.
fn retag_owner(text: &str, from: &str, count: usize, to: &str) -> Option<String> {
    let root_open = text.find('{')?;
    let (root, _) = members(text, root_open)?;
    if member(text, root.first()?) != "1" {
        return None;
    }
    let owner_open = list_open(text, root.get(1)?)?;
    let (owner, _) = members(text, owner_open)?;
    if owner.len() != count || member(text, &owner[0]) != from {
        return None;
    }
    let tag = trimmed(text, &owner[0]);
    Some(format!("{}{to}{}", &text[..tag.start], &text[tag.end..]))
}

/// Every rule that reads a record by its shape alone (no kind needed), in the
/// order they apply; `None` when none does.
///
/// A code-3 record of one `{2,…}` header block is a common picture (the
/// older of its two layouts, see `metadata_source_for_text`): it is upgraded
/// whole, before the header rule would make it read as a Style's.
pub(super) fn upgrade_record_by_shape(text: &str, uuid: &str) -> Option<String> {
    if is_code3_picture_with_two_header(text) {
        let text = upgrade_header_blocks(text)?;
        return upgrade_common_picture(&text);
    }
    let upgraded = upgrade_header_blocks(text);
    let text_now = upgraded.as_deref().unwrap_or(text);
    let upgraded = upgrade_root_section_identities(text_now, uuid).or(upgraded);
    let text_now = upgraded.as_deref().unwrap_or(text);
    upgrade_form_record(text_now).or(upgraded)
}

/// A form's metadata record `{12,<header>,…}` (5 members; an owned form under
/// `{0,…}`, a common form under `{4,…}`) -> `{13,…}`, members unchanged.
///
/// Evidence: re-serialized by 8.3.27, the 93 forms of two real
/// configurations stored as 12 came back as 13 with nothing else changed but
/// their header (upgraded on its own); the 2 common forms likewise.
fn upgrade_form_record(text: &str) -> Option<String> {
    let root_open = text.find('{')?;
    let (root, _) = members(text, root_open)?;
    if root.len() != 3 || member(text, &root[0]) != "1" {
        return None;
    }
    let wrapper_open = list_open(text, &root[1])?;
    let (mut wrapper, _) = members(text, wrapper_open)?;
    // One form record of 1C:Документооборот sits a level deeper,
    // `{1,{0,{12,…}},{0}}`.
    if wrapper.len() == 3 && member(text, &wrapper[0]) == "1" {
        let inner_open = list_open(text, &wrapper[1])?;
        wrapper = members(text, inner_open)?.0;
        if member(text, wrapper.first()?) != "0" {
            return None;
        }
    }
    if !matches!(member(text, wrapper.first()?), "0" | "4") {
        return None;
    }
    let form_open = list_open(text, wrapper.get(1)?)?;
    let (form, _) = members(text, form_open)?;
    if form.len() != 5
        || member(text, &form[0]) != "12"
        || !member(text, &form[1]).starts_with("{3,")
        || list_open(text, &form[4]).is_none()
    {
        return None;
    }
    let tag = trimmed(text, &form[0]);
    Some(format!("{}13{}", &text[..tag.start], &text[tag.end..]))
}

fn is_code3_picture_with_two_header(text: &str) -> bool {
    (|| {
        let root_open = text.find('{')?;
        let (root, _) = members(text, root_open)?;
        let owner_open = list_open(text, root.get(1)?)?;
        let (owner, _) = members(text, owner_open)?;
        let header = member(text, owner.get(1)?);
        Some(
            root.len() == 3
                && member(text, &root[0]) == "1"
                && owner.len() == 2
                && member(text, &owner[0]) == "3"
                && header.starts_with("{2,"),
        )
    })()
    .unwrap_or(false)
}

/// Common picture `{3,<header>}` -> `{4,<header>,0,0}`: the two availability
/// flags the older record has no member for read as unset.
///
/// Evidence: every such picture of three real configurations prints
/// `AvailabilityForChoice`/`AvailabilityForAppearance` false in the platform's
/// dump (901 + 673 + 12); re-serialized by 8.3.27 the 12 came back as
/// `{4,<header>,0,0}`. Once the header is upgraded (`{3,{2,…}}` or
/// `{3,{1,{0,0,…}}}` stored) the record reads as a Style's by its code, so
/// the rule is keyed by the kind the configuration root states.
fn upgrade_common_picture(text: &str) -> Option<String> {
    let root_open = text.find('{')?;
    let (root, _) = members(text, root_open)?;
    if root.len() != 3 || member(text, &root[0]) != "1" || member(text, &root[2]) != "0" {
        return None;
    }
    let owner_open = list_open(text, &root[1])?;
    let (owner, owner_close) = members(text, owner_open)?;
    if owner.len() != 2 || member(text, &owner[0]) != "3" || list_open(text, &owner[1]).is_none() {
        return None;
    }
    let tag = trimmed(text, &owner[0]);
    let mut out = String::with_capacity(text.len() + 8);
    out.push_str(&text[..tag.start]);
    out.push('4');
    out.push_str(&text[tag.end..owner_close]);
    out.push_str(",0,0");
    out.push_str(&text[owner_close..]);
    Some(out)
}

/// Exchange plan `{35,…}` (49 members) -> `{36,…}`
/// (50), the oldest version the reader knows: member 49 is appended as `0`.
///
/// Evidence: in seven real configurations re-serialized by 8.3.27 every `{35,…}` plan (30)
/// became `{37,…}` with members 49, 50 = `0`, `1` and the others unchanged;
/// every `{36,…}` plan (21) kept member 49 = `0` and gained member 50 = `1`.
/// The nested records (attributes `{3,…}`, standard attribute bags
/// `{13,24,…}`) are the ones a `{36,…}` plan keeps in real
/// configurations, which the reader already reads. The platform loads the fixture
/// `upgrade/exchange_plan/v35.cf` to the XML of the original.
fn upgrade_exchange_plan(text: &str) -> Option<String> {
    let root_open = text.find('{')?;
    let (root, _) = members(text, root_open)?;
    if root.len() != 8 || member(text, &root[0]) != "1" || member(text, &root[2]) != "5" {
        return None;
    }
    let owner_open = list_open(text, &root[1])?;
    let (owner, owner_close) = members(text, owner_open)?;
    if member(text, &owner[0]) != "35" || owner.len() != 49 {
        return None;
    }
    let tag = trimmed(text, &owner[0]);
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(&text[..tag.start]);
    out.push_str("36");
    out.push_str(&text[tag.end..owner_close]);
    out.push_str(",0");
    out.push_str(&text[owner_close..]);
    Some(out)
}

const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// Every metadata header block in the older spelling rewritten into the
/// current one, or `None` when the text holds none.
///
/// Older spellings, after `<Name>,<Synonym>,<Comment>`:
/// - `{1,{0,0,<uuid>},…,0,<k>,<k (uuid, state) pairs>}` and the same with
///   `{1,0,<uuid>}`;
/// - `{2,{1,0,<uuid>},…,0,<k>,<pairs>,<nil>}`;
/// - `{0,{0,0,<uuid>},…}` (nothing after the comment).
///
/// Current: `{3,{1,0,<uuid>},<Name>,<Synonym>,<Comment>,0,0,<nil>,0}` -- the
/// only spelling 8.3.27 writes (seven configurations it re-serialized hold
/// no other).
///
/// Evidence: real configurations as stored and re-serialized by 8.3.27
/// (their native XML loaded back and saved): the old shapes, 23 000 blocks
/// in three of them, all came back in the current spelling with name,
/// synonym and comment unchanged and no pairs (property states the XML does
/// not carry -- the platform dumps nothing of them).
pub(super) fn upgrade_header_blocks(text: &str) -> Option<String> {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(",0,") {
        // An identity `{0,0,` or `{1,0,`: the digit before the found `,0,`.
        let at = from + found;
        from = at + 1;
        if at < 2 || !matches!(&text.as_bytes()[at - 2..at], b"{0" | b"{1") {
            continue;
        }
        let at = at - 2;
        if let Some(edit) = header_block_at(text, at)
            && edits
                .last()
                .is_none_or(|(last, _)| last.end <= edit.0.start)
        {
            from = edit.0.end;
            edits.push(edit);
        }
    }
    if edits.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len() + edits.len() * 48);
    let mut done = 0;
    for (range, replacement) in edits {
        out.push_str(&text[done..range.start]);
        out.push_str(&replacement);
        done = range.end;
    }
    out.push_str(&text[done..]);
    Some(out)
}

/// The configuration root record `uuid` with its sections' object identities
/// `{0,0,<id>}` in the current spelling `{1,0,<id>}`, or `None`.
///
/// A root saved by an older platform (compatibility 8.3.9, container
/// version 216) spells them `{0,0,…}`; the platform's own dump of it names
/// the same ids as `<xr:ObjectId>` (and every root 8.3.27 writes spells them
/// `{1,0,…}`). Run after [`upgrade_header_blocks`], so the only `{0,0,<uuid>}`
/// left in a root are these identities.
pub(super) fn upgrade_root_section_identities(text: &str, uuid: &str) -> Option<String> {
    let body = text.trim_start_matches('\u{feff}').trim_start();
    let rest = body.strip_prefix("{2,")?.trim_start();
    let rest = rest.strip_prefix('{')?.trim_start();
    if !rest.starts_with(uuid) || !rest[uuid.len()..].trim_start().starts_with('}') {
        return None;
    }
    const OLD: &str = "{0,0,";
    let mut out = String::with_capacity(text.len());
    let mut done = 0;
    let mut from = 0;
    while let Some(found) = text[from..].find(OLD) {
        let at = from + found;
        from = at + OLD.len();
        let id_end = from + 36;
        if text.get(from..id_end).is_some_and(is_uuid) && text.as_bytes().get(id_end) == Some(&b'}')
        {
            out.push_str(&text[done..at]);
            out.push_str("{1,0,");
            done = from;
        }
    }
    if done == 0 {
        return None;
    }
    out.push_str(&text[done..]);
    Some(out)
}

/// The header block whose identity member `{0,0,<uuid>}` opens at `at`: its
/// range and its current spelling.
fn header_block_at(text: &str, at: usize) -> Option<(Range<usize>, String)> {
    let bytes = text.as_bytes();
    // Back over `<tag>,` and whitespace to the block's own opening brace.
    let mut index = at;
    let skip_ws = |mut index: usize| {
        while index > 0 && bytes[index - 1].is_ascii_whitespace() {
            index -= 1;
        }
        index
    };
    index = skip_ws(index);
    if index == 0 || bytes[index - 1] != b',' {
        return None;
    }
    index = skip_ws(index - 1);
    let tag_end = index;
    while index > 0 && bytes[index - 1].is_ascii_digit() {
        index -= 1;
    }
    let tag = &text[index..tag_end];
    index = skip_ws(index);
    if index == 0 || bytes[index - 1] != b'{' || !matches!(tag, "0" | "1" | "2") {
        return None;
    }
    let open = index - 1;
    let (fields, close) = members(text, open)?;
    let identity_open = list_open(text, fields.get(1)?)?;
    if identity_open != at {
        return None;
    }
    let (identity, _) = members(text, identity_open)?;
    let uuid = member(text, identity.get(2)?);
    if identity.len() != 3 || member(text, &identity[1]) != "0" || !is_uuid(uuid) {
        return None;
    }
    let old_identity = member(text, &identity[0]) == "0";
    if matches!(tag, "0" | "2") && old_identity == (tag == "2") {
        // `{0,{0,0,…}` and `{2,{1,0,…}` only.
        return None;
    }
    let name = member(text, fields.get(2)?);
    let synonym = member(text, fields.get(3)?);
    let comment = member(text, fields.get(4)?);
    if !is_quoted(name) || !synonym.starts_with('{') || !is_quoted(comment) {
        return None;
    }
    // `{2,…}` closes its pairs with the nil uuid.
    let trailing = usize::from(tag == "2");
    let well_formed = match (tag, fields.len()) {
        ("0", 5) => true,
        ("1" | "2", len) if len >= 7 + trailing && member(text, &fields[5]) == "0" => {
            let pairs = member(text, &fields[6]).parse::<usize>().ok()?;
            len == pairs.checked_mul(2)?.checked_add(7 + trailing)?
                && (trailing == 0 || member(text, &fields[len - 1]) == NIL_UUID)
                && (0..pairs).all(|pair| {
                    is_uuid(member(text, &fields[7 + 2 * pair]))
                        && member(text, &fields[8 + 2 * pair])
                            .bytes()
                            .all(|byte| byte.is_ascii_digit())
                })
        }
        _ => false,
    };
    well_formed.then(|| {
        (
            open..close + 1,
            format!("{{3,{{1,0,{uuid}}},{name},{synonym},{comment},0,0,{NIL_UUID},0}}"),
        )
    })
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn is_quoted(value: &str) -> bool {
    value.len() >= 2 && value.starts_with('"') && value.ends_with('"')
}

/// The byte ranges of the members of the brace list opening at `open`
/// (separators excluded, surrounding whitespace kept) and the index of its
/// closing brace.
fn members(text: &str, open: usize) -> Option<(Vec<Range<usize>>, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'{') {
        return None;
    }
    let mut out = Vec::new();
    let (mut depth, mut start, mut quoted) = (0usize, open + 1, false);
    let mut index = open + 1;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 1;
                } else {
                    quoted = false;
                }
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' => depth += 1,
                b'}' if depth == 0 => {
                    out.push(start..index);
                    return Some((out, index));
                }
                b'}' => depth -= 1,
                b',' if depth == 0 => {
                    out.push(start..index);
                    start = index + 1;
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

fn trimmed(text: &str, range: &Range<usize>) -> Range<usize> {
    let slice = &text[range.clone()];
    let start = range.start + (slice.len() - slice.trim_start().len());
    let end = range.end - (slice.len() - slice.trim_end().len());
    start..end.max(start)
}

fn member<'a>(text: &'a str, range: &Range<usize>) -> &'a str {
    &text[trimmed(text, range)]
}

/// The opening brace of a member that is itself a list.
fn list_open(text: &str, range: &Range<usize>) -> Option<usize> {
    let range = trimmed(text, range);
    (text.as_bytes().get(range.start) == Some(&b'{')).then_some(range.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(tag: &str, count: usize) -> String {
        let owner = std::iter::once(tag.to_string())
            .chain((1..count).map(|index| format!("{{\"a,}}\",{index}}}")))
            .collect::<Vec<_>>()
            .join(",\r\n");
        format!("{{1,\r\n{{{owner}}},5,{{a}},{{b}},{{c}},{{d}},{{e}}}}")
    }

    #[test]
    fn a_35_exchange_plan_gains_member_49() {
        let upgraded = upgrade_metadata_record("ExchangePlan", &plan("35", 49)).unwrap();
        let mut expected = plan("36", 49);
        let close = expected.find("},5,").unwrap();
        expected.insert_str(close, ",0");
        assert_eq!(upgraded, expected);
    }

    const U: &str = "0133535f-082a-4e61-a891-4b939fd004a0";
    const P: &str = "32e087ab-1491-49b6-aba7-43571b41ac2b";

    #[test]
    fn an_older_header_block_reads_in_the_current_spelling() {
        let old = format!(
            "{{1,\r\n{{1,\r\n{{2,\r\n{{1,\r\n{{0,0,{U}}},\"Имя\",\r\n{{1,\"ru\",\"Имя, \"\"да\"\"\"}},\"\",0,0}},\r\n{{\"Pattern\"}}}}}},0}}"
        );
        let new = format!(
            "{{1,\r\n{{1,\r\n{{2,\r\n{{3,{{1,0,{U}}},\"Имя\",{{1,\"ru\",\"Имя, \"\"да\"\"\"}},\"\",0,0,{NIL_UUID},0}},\r\n{{\"Pattern\"}}}}}},0}}"
        );
        assert_eq!(upgrade_header_blocks(&old).unwrap(), new);
        // Property states are not carried: the platform prints nothing of them.
        let with_states = format!("{{12,{{1,{{0,0,{U}}},\"М\",{{0}},\"к\",0,1,{P},3}},1,0}}");
        assert_eq!(
            upgrade_header_blocks(&with_states).unwrap(),
            format!("{{12,{{3,{{1,0,{U}}},\"М\",{{0}},\"к\",0,0,{NIL_UUID},0}},1,0}}")
        );
        let two = format!("{{2,{{1,0,{U}}},\"Ф\",{{0}},\"\",0,1,{P},3,{NIL_UUID}}}");
        assert_eq!(
            upgrade_header_blocks(&two).unwrap(),
            format!("{{3,{{1,0,{U}}},\"Ф\",{{0}},\"\",0,0,{NIL_UUID},0}}")
        );
        let one_new = format!("{{1,{{1,0,{U}}},\"Ф\",{{0}},\"\",0,0}}");
        assert_eq!(
            upgrade_header_blocks(&one_new).unwrap(),
            format!("{{3,{{1,0,{U}}},\"Ф\",{{0}},\"\",0,0,{NIL_UUID},0}}")
        );
        let short = format!("{{0,{{0,0,{U}}},\"У\",{{1,\"ru\",\"У\"}},\"\"}}");
        assert_eq!(
            upgrade_header_blocks(&short).unwrap(),
            format!("{{3,{{1,0,{U}}},\"У\",{{1,\"ru\",\"У\"}},\"\",0,0,{NIL_UUID},0}}")
        );
    }

    #[test]
    fn a_current_or_unrecognized_header_is_left_as_stored() {
        let current = format!("{{3,{{1,0,{U}}},\"И\",{{0}},\"\",0,0,{NIL_UUID},0}}");
        assert_eq!(upgrade_header_blocks(&current), None);
        // Pair count disagreeing with the members, a state that is no number,
        // an identity that is no header's.
        for text in [
            format!("{{1,{{0,0,{U}}},\"И\",{{0}},\"\",0,2,{P},3}}"),
            format!("{{1,{{0,0,{U}}},\"И\",{{0}},\"\",0,1,{P},x}}"),
            format!("{{1,{{0,0,{U}}},И,{{0}},\"\",0,0}}"),
            format!("{{5,{{0,0,{U}}},\"И\",{{0}},\"\",0,0}}"),
            // `{2,…}` without its closing nil, with the old identity.
            format!("{{2,{{1,0,{U}}},\"И\",{{0}},\"\",0,0}}"),
            format!("{{2,{{0,0,{U}}},\"И\",{{0}},\"\",0,0,{NIL_UUID}}}"),
            format!("{{0,{{1,0,{U}}},\"И\",{{0}},\"\"}}"),
            "{0,0,0}".to_string(),
            // A multi-byte character right before `,0,` (a panic once).
            "{€,0,0}".to_string(),
        ] {
            assert_eq!(upgrade_header_blocks(&text), None, "{text}");
        }
    }

    #[test]
    fn a_3_common_picture_gains_its_two_flags() {
        let old = format!(
            "{{1,\r\n{{3,\r\n{{3,{{1,0,{U}}},\"К\",{{0}},\"\",0,0,{NIL_UUID},0}}\r\n}},0}}"
        );
        let new = format!(
            "{{1,\r\n{{4,\r\n{{3,{{1,0,{U}}},\"К\",{{0}},\"\",0,0,{NIL_UUID},0}}\r\n,0,0}},0}}"
        );
        assert_eq!(upgrade_metadata_record("CommonPicture", &old).unwrap(), new);
        // A current picture, and a style of the same shape, are left alone.
        let current =
            format!("{{1,{{4,{{3,{{1,0,{U}}},\"К\",{{0}},\"\",0,0,{NIL_UUID},0}},1,0}},0}}");
        assert_eq!(upgrade_metadata_record("CommonPicture", &current), None);
        assert_eq!(upgrade_metadata_record("Style", &old), None);
    }

    #[test]
    fn a_12_form_record_reads_as_13() {
        let old = format!(
            "{{1,\r\n{{0,\r\n{{12,\r\n{{1,\r\n{{0,0,{U}}},\"Ф\",\r\n{{0}},\"\",0,1,{P},3}},0,1,\r\n{{2,a,b}}\r\n}}\r\n}},0}}"
        );
        let new = format!(
            "{{1,\r\n{{0,\r\n{{13,\r\n{{3,{{1,0,{U}}},\"Ф\",{{0}},\"\",0,0,{NIL_UUID},0}},0,1,\r\n{{2,a,b}}\r\n}}\r\n}},0}}"
        );
        assert_eq!(upgrade_record_by_shape(&old, U).unwrap(), new);
        let common = old.replace("{1,\r\n{0,\r\n{12,", "{1,\r\n{4,\r\n{12,");
        assert!(
            upgrade_record_by_shape(&common, U)
                .unwrap()
                .starts_with("{1,\r\n{4,\r\n{13,")
        );
        // Another wrapper, or a count that is no form's, keeps its tag.
        let other = old.replace("{1,\r\n{0,\r\n{12,", "{1,\r\n{7,\r\n{12,");
        assert!(upgrade_record_by_shape(&other, U).unwrap().contains("{12,"));
    }

    /// Evidence: 1C:Документооборот stores one form record one level deeper,
    /// `{1,{1,{0,{12,…}},{0}},0}`; re-serialized by 8.3.27 it comes back as
    /// `{1,{1,{0,{13,…}},{0}},0}`, like its five siblings stored as 13.
    #[test]
    fn a_nested_12_form_record_reads_as_13() {
        let old = format!(
            "{{1,\r\n{{1,\r\n{{0,\r\n{{12,\r\n{{1,\r\n{{0,0,{U}}},\"Ф\",\r\n{{0}},\"\",0,1,{P},3}},0,1,\r\n{{2,a,b}}\r\n}}\r\n}},\r\n{{0}}\r\n}},0}}"
        );
        let upgraded = upgrade_record_by_shape(&old, U).unwrap();
        assert!(
            upgraded.starts_with("{1,\r\n{1,\r\n{0,\r\n{13,"),
            "{upgraded}"
        );
    }

    #[test]
    fn a_20_subsystem_reads_as_21() {
        let old = "{1,\r\n{20,{h},1,\r\n{0,0},1,{p},{e},{c}},\r\n{a},0}";
        assert_eq!(
            upgrade_metadata_record("Subsystem", old).unwrap(),
            "{1,\r\n{21,{h},1,\r\n{0,0},1,{p},{e},{c}},\r\n{a},0}"
        );
        assert_eq!(
            upgrade_metadata_record("Subsystem", &old.replace("{20,", "{21,")),
            None
        );
        assert_eq!(
            upgrade_metadata_record("Subsystem", &old.replace(",{c}}", "}")),
            None
        );
    }

    #[test]
    fn other_versions_and_kinds_are_left_as_stored() {
        assert_eq!(
            upgrade_metadata_record("ExchangePlan", &plan("36", 50)),
            None
        );
        assert_eq!(
            upgrade_metadata_record("ExchangePlan", &plan("37", 51)),
            None
        );
        assert_eq!(
            upgrade_metadata_record("ExchangePlan", &plan("35", 48)),
            None
        );
        assert_eq!(
            upgrade_metadata_record("ChartOfCalculationTypes", &plan("35", 49)),
            None
        );
    }
}
