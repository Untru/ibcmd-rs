//! The `root` row of the configuration: `{2,<uuid of the configuration>,<payload>}`.
//!
//! On 8.3.27 the payload is empty and a native import writes the row as it is, so a staged `root` that differs
//! from the stored one names another configuration. The 8.5 platform (measured on 8.5.1.1150) writes a payload
//! of 128 bytes, base64 in wrapped lines, that is encrypted in blocks of 16 bytes chained one to the next: it
//! **re-stamps the final block on every write**, an import that stages one unchanged descriptor included, and
//! a native apply then promotes the staged row. The seven blocks before it, and the uuid, do not change. A
//! chained block depends on the plaintext up to itself only, so a change of the row's substance (which
//! configuration it names, what the payload encrypts) would show in the header or in those blocks; the final
//! block alone is the platform's stamp.

use ibcmd_1c_text::decode_base64_mime_lenient as decode_base64_mime;

/// The size of one encrypted block of the payload.
const BLOCK: usize = 16;

/// How the staged `root` row relates to the stored one.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RootRelation {
    /// The very same bytes.
    Same,
    /// The same configuration and the same payload but for its final block.
    Restamped,
    /// Anything else: another configuration, another payload, a row that is not read.
    Different,
}

/// The parts of a `root` row: the format tag, the configuration uuid (lower case) and the decoded payload.
fn parts(row: &[u8]) -> Option<(&str, String, Vec<u8>)> {
    let row = row.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(row);
    let text = std::str::from_utf8(row).ok()?.trim();
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    let mut fields = inner.splitn(3, ',');
    let tag = fields.next()?.trim();
    let uuid = fields.next()?.trim().to_ascii_lowercase();
    let payload = decode_base64_mime(fields.next()?)?;
    Some((tag, uuid, payload))
}

/// Compares the staged row with the stored one.
pub(super) fn relation(stored: &[u8], staged: &[u8]) -> RootRelation {
    if stored == staged {
        return RootRelation::Same;
    }
    let (Some((old_tag, old_uuid, old_payload)), Some((new_tag, new_uuid, new_payload))) =
        (parts(stored), parts(staged))
    else {
        return RootRelation::Different;
    };
    let stamped = old_payload.len() >= 2 * BLOCK
        && old_payload.len() == new_payload.len()
        && old_payload.len() % BLOCK == 0;
    if old_tag == new_tag && old_uuid == new_uuid && stamped {
        let head = old_payload.len() - BLOCK;
        if old_payload[..head] == new_payload[..head] {
            return RootRelation::Restamped;
        }
    }
    RootRelation::Different
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "66193438-abc5-410b-a1f1-a204102d1a62";

    /// A payload of `blocks` blocks: block `i` is filled with the byte `i`, base64 in lines of 64 characters.
    fn payload(blocks: usize, last: u8) -> String {
        let mut bytes = Vec::new();
        for block in 0..blocks {
            let fill = if block + 1 == blocks {
                last
            } else {
                block as u8
            };
            bytes.extend_from_slice(&[fill; BLOCK]);
        }
        let mut alphabet = String::new();
        for chunk in bytes.chunks(3) {
            let value = chunk
                .iter()
                .enumerate()
                .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..4 {
                if i > chunk.len() {
                    alphabet.push('=');
                } else {
                    let index = ((value >> (18 - 6 * i)) & 0x3F) as u8;
                    alphabet.push(match index {
                        0..=25 => (b'A' + index) as char,
                        26..=51 => (b'a' + index - 26) as char,
                        52..=61 => (b'0' + index - 52) as char,
                        62 => '+',
                        _ => '/',
                    });
                }
            }
        }
        alphabet
            .as_bytes()
            .chunks(64)
            .map(|line| std::str::from_utf8(line).unwrap())
            .collect::<Vec<_>>()
            .join("\r\n")
    }

    fn row(uuid: &str, payload: &str) -> Vec<u8> {
        format!("{{2,{uuid},{payload}}}").into_bytes()
    }

    #[test]
    fn the_same_bytes_are_the_same_row() {
        let stored = row(UUID, &payload(8, 7));
        assert_eq!(relation(&stored, &stored), RootRelation::Same);
    }

    #[test]
    fn a_payload_that_differs_in_its_final_block_only_is_a_restamp() {
        let stored = row(UUID, &payload(8, 7));
        let staged = row(UUID, &payload(8, 200));
        assert_ne!(stored, staged);
        assert_eq!(relation(&stored, &staged), RootRelation::Restamped);
        // with a byte order mark and other line breaks
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend(row(UUID, &payload(8, 200).replace("\r\n", "\n")));
        assert_eq!(relation(&stored, &bom), RootRelation::Restamped);
    }

    #[test]
    fn a_change_before_the_final_block_is_another_row() {
        let stored = row(UUID, &payload(8, 7));
        // a character of the third block differs
        let mut other = payload(8, 7);
        let flipped = if other.as_bytes()[60] == b'A' {
            "B"
        } else {
            "A"
        };
        other.replace_range(60..61, flipped);
        assert_eq!(
            relation(&stored, &row(UUID, &other)),
            RootRelation::Different
        );
    }

    #[test]
    fn another_uuid_or_another_length_or_another_format_is_another_row() {
        let stored = row(UUID, &payload(8, 7));
        let other_uuid = row("00000000-abc5-410b-a1f1-a204102d1a62", &payload(8, 200));
        assert_eq!(relation(&stored, &other_uuid), RootRelation::Different);
        assert_eq!(
            relation(&stored, &row(UUID, &payload(7, 200))),
            RootRelation::Different
        );
        let tagged = format!("{{3,{UUID},{}}}", payload(8, 200)).into_bytes();
        assert_eq!(relation(&stored, &tagged), RootRelation::Different);
        assert_eq!(relation(&stored, b"another"), RootRelation::Different);
    }

    #[test]
    fn a_row_without_a_payload_differs_or_not_as_bytes() {
        // 8.3.27: the payload is empty, nothing is stamped
        let stored = row(UUID, "");
        assert_eq!(relation(&stored, &stored), RootRelation::Same);
        assert_eq!(
            relation(&stored, &row("00000000-abc5-410b-a1f1-a204102d1a62", "")),
            RootRelation::Different
        );
        // a payload of one block has no block before the stamped one
        assert_eq!(
            relation(&row(UUID, &payload(1, 1)), &row(UUID, &payload(1, 2))),
            RootRelation::Different
        );
    }
}
