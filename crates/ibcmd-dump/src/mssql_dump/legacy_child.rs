//! Older metadata child records respelled in the current layout.
//!
//! A typed child (attribute, resource, dimension) carries a common record
//! `{27, {2, <header>, <pattern>}, …}` of 23 members. Version 27 information
//! registers of one 8.3.27 corpus still store `{25, …}` of 21 members: the
//! same slots without the trailing create-on-input and choice-history-on-input
//! codes, which 8.3.27.2214 dumps as `Auto` (`0`) for each such child. The
//! header block is left to `upgrade::upgrade_header_blocks`, which runs first.
//! Rewriting the old record once lets every parser keep its one layout.

use super::split_1c_braced_fields;
use ibcmd_schema::metadata_child_storage_facts::LegacyTypedMetadataChildLayout;

/// Byte offset just past the brace that closes the one opening at `start`.
fn braced_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = start;
    let mut in_string = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 1;
                } else {
                    in_string = false;
                }
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

/// `{25, …}` rewritten as `{27, …}`, or `None` when the record is not one.
fn upgrade_record(record: &str) -> Option<String> {
    let fields = split_1c_braced_fields(record, 0)?;
    let layout = LegacyTypedMetadataChildLayout::from_fields(&fields)?;
    let typed = split_1c_braced_fields(fields.get(layout.typed_payload_slot)?.trim(), 0)?;
    if !LegacyTypedMetadataChildLayout::typed_payload_is_valid(&typed) {
        return None;
    }
    if !typed.get(1)?.trim().starts_with('{') {
        return None;
    }
    let typed = fields.get(layout.typed_payload_slot)?.trim().to_string();
    let mut members = vec![layout.current_revision.to_string(), typed];
    members.extend(
        fields[layout.following_members_slot..]
            .iter()
            .map(|field| field.to_string()),
    );
    members.extend(
        layout
            .appended_defaults
            .iter()
            .map(|field| field.to_string()),
    );
    Some(format!("{{{}}}", members.join(",")))
}

/// The text with every old child record upgraded; `None` when none is found.
pub(super) fn upgrade_legacy_child_records(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut cursor = 0;
    let mut changed = false;
    let mut in_string = false;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"' {
            if in_string && bytes.get(cursor + 1) == Some(&b'"') {
                cursor += 2;
                continue;
            }
            in_string = !in_string;
            cursor += 1;
            continue;
        }
        if in_string || !bytes[cursor..].starts_with(LegacyTypedMetadataChildLayout::RECORD_START) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        let Some(end) = braced_end(text, start) else {
            break;
        };
        if let Some(upgraded) = upgrade_record(&text[start..end]) {
            out.push_str(&text[copied..start]);
            out.push_str(&upgraded);
            copied = end;
            cursor = end;
            changed = true;
        } else {
            cursor = start + 1;
        }
    }
    if !changed {
        return None;
    }
    out.push_str(&text[copied..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::upgrade_legacy_child_records;

    #[test]
    fn upgrades_version_25_child_record() {
        let old = "{5,{25,{2,{3,{1,0,a},\"N\",{1,\"ru\",\"N\"},\"\",0,0,n,0},{\"Pattern\"}},\
                   0,{0},{0},0,\"\",0,{\"U\"},{\"U\"},0,z,2,0,{5004,0},{3,0,0},{0,0},0,{0},{\"U\"},0},0,1}";
        let new = upgrade_legacy_child_records(old).unwrap();
        assert_eq!(
            new,
            "{5,{27,{2,{3,{1,0,a},\"N\",{1,\"ru\",\"N\"},\"\",0,0,n,0},{\"Pattern\"}},\
             0,{0},{0},0,\"\",0,{\"U\"},{\"U\"},0,z,2,0,{5004,0},{3,0,0},{0,0},0,{0},{\"U\"},0,0,0},0,1}"
        );
    }

    #[test]
    fn leaves_other_records_alone() {
        assert!(upgrade_legacy_child_records("{25,1,2}").is_none());
    }

    #[test]
    fn leaves_record_shaped_text_inside_quoted_comments_unchanged() {
        let record = format!("{{25,{{2,{{3}},{{Pattern}}}},{}}}", ["0"; 19].join(","));
        assert!(upgrade_legacy_child_records(&record).is_some());
        for comment in [
            format!(r#"{{1,"{record}"}}"#),
            format!(r#"{{1,"prefix ""quoted"" {record} suffix"}}"#),
        ] {
            assert!(upgrade_legacy_child_records(&comment).is_none());
        }
    }

    #[test]
    fn upgrades_nested_records_after_escaped_quoted_comments() {
        let record = format!("{{25,{{2,{{3}},{{Pattern}}}},{}}}", ["0"; 19].join(","));
        let prefix = format!(r#"{{1,"prefix ""quoted"" {record}",{{5,"#);
        let text = format!("{prefix}{record},0,1}}}}");
        let upgraded = upgrade_legacy_child_records(&text).unwrap();
        assert!(upgraded.starts_with(&prefix));
        assert_eq!(upgraded.matches("{25,").count(), 1);
        assert_eq!(upgraded.matches("{27,").count(), 1);
        assert!(upgraded.ends_with(",0,0},0,1}}"));
    }
}
