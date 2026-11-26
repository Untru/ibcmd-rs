//! Scalar fields of 1C braced values: UUIDs, `0`/`1` flags, unsigned
//! numbers, `YYYYMMDDhhmmss` dates, and the payload of a `{#base64:...}`
//! value, read and written; and the scan of a text for the UUIDs it names.

use std::collections::BTreeSet;

pub fn parse_uuid_field(value: &str) -> Option<String> {
    let value = value.trim();
    if is_uuid_text(value) {
        Some(value.to_string())
    } else {
        None
    }
}

pub fn parse_non_zero_uuid(value: &str) -> Option<String> {
    let uuid = parse_uuid_field(value)?;
    if uuid == "00000000-0000-0000-0000-000000000000" {
        None
    } else {
        Some(uuid)
    }
}

pub fn is_uuid_text(value: &str) -> bool {
    value.len() == 36 && value.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

pub fn parse_1c_bool_field(value: Option<&str>) -> Option<bool> {
    parse_1c_bool_flag(value?.trim())
}

// An absent legacy field can have a documented default. A present value
// outside the Boolean domain is a conversion refusal, never that default.
pub fn parse_1c_bool_field_or_default(value: Option<&str>, default: bool) -> Option<bool> {
    value.map_or(Some(default), |value| parse_1c_bool_field(Some(value)))
}

pub fn parse_1c_u32_field(value: Option<&str>) -> Option<u32> {
    value?.trim().parse().ok()
}

pub fn parse_1c_bool_flag(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub fn extract_base64_payload(text: &str) -> Option<&str> {
    let prefix = "{#base64:";
    let start = text.find(prefix)? + prefix.len();
    let end = text[start..].find('}')? + start;
    Some(&text[start..end])
}

pub fn uuid_like_values(text: &str) -> BTreeSet<String> {
    uuid_like_values_in_text_order(text).into_iter().collect()
}

pub fn uuid_like_values_in_text_order(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut values = Vec::new();
    let mut seen = BTreeSet::new();
    if bytes.len() < 36 {
        return values;
    }
    for start in 0..=bytes.len() - 36 {
        let value = &bytes[start..start + 36];
        if is_uuid_like_ascii(value) {
            let value = String::from_utf8_lossy(value).to_ascii_lowercase();
            if seen.insert(value.clone()) {
                values.push(value);
            }
        }
    }
    values
}

pub fn is_uuid_like_ascii(value: &[u8]) -> bool {
    if value.len() != 36 {
        return false;
    }
    for (index, byte) in value.iter().copied().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub fn decode_base64_mime(input: &str) -> Option<Vec<u8>> {
    let values = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if values.len() % 4 != 0 {
        return None;
    }

    let mut output = Vec::with_capacity(values.len() / 4 * 3);
    for chunk in values.chunks(4) {
        let mut decoded = [0u8; 4];
        let mut padding = 0usize;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if byte == b'=' {
                padding += 1;
                decoded[index] = 0;
                continue;
            }
            if padding > 0 {
                return None;
            }
            decoded[index] = base64_value(byte)?;
        }
        if padding > 2 {
            return None;
        }
        output.push((decoded[0] << 2) | (decoded[1] >> 4));
        if padding < 2 {
            output.push((decoded[1] << 4) | (decoded[2] >> 2));
        }
        if padding < 1 {
            output.push((decoded[2] << 6) | decoded[3]);
        }
    }

    Some(output)
}

/// [`decode_base64_mime`], but a group of four padded by three or four `=`
/// is read too, as one byte (`Q===` is `0x40`, `====` is `0`), where
/// `decode_base64_mime` answers `None`.
///
/// This is `module_blob`'s decoder, which the root reads stored and
/// imported payloads with (as `module_blob::decode_base64_mime`), and the
/// one `ibcmd-form-body`'s record reader reads a record's
/// `{#base64:...}` fields with; it stays as lenient as it was there.
pub fn decode_base64_mime_lenient(input: &str) -> Option<Vec<u8>> {
    let values = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if values.len() % 4 != 0 {
        return None;
    }

    let mut output = Vec::with_capacity(values.len() / 4 * 3);
    for chunk in values.chunks(4) {
        let mut decoded = [0u8; 4];
        let mut padding = 0usize;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if byte == b'=' {
                padding += 1;
                decoded[index] = 0;
                continue;
            }
            if padding > 0 {
                return None;
            }
            decoded[index] = base64_value(byte)?;
        }
        output.push((decoded[0] << 2) | (decoded[1] >> 4));
        if padding < 2 {
            output.push((decoded[1] << 4) | (decoded[2] >> 2));
        }
        if padding < 1 {
            output.push((decoded[2] << 6) | decoded[3]);
        }
    }
    Some(output)
}

/// The six bits one base64 character (RFC 4648 alphabet) stands for; `None`
/// for `=` and any other byte.
pub fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Standard base64 (RFC 4648 alphabet, `=` padding, no line breaks).
pub fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(third & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

/// A `{#base64:...}` payload laid out as the platform stores it: 64
/// characters per line, each full line ended by `\r\r\n` -- so a payload whose
/// length is a multiple of 64 ends with the break too (help and picture rows
/// of both corpora).
///
/// That last break is the platform's current writer: of the payloads whose
/// length is a multiple of 64, 32 of 32 БСП and 362 of 384 ERP УХ help
/// payloads carry it, and 31 of 34 and 174 of 184 common pictures. The rest
/// were written by an older writer the source does not tell apart.
pub fn platform_base64_token(bytes: &[u8]) -> String {
    let encoded = encode_base64(bytes);
    let mut token = String::with_capacity(encoded.len() + encoded.len() / 64 * 3 + 10);
    token.push_str("{#base64:");
    for line in encoded.as_bytes().chunks(64) {
        token.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        if line.len() == 64 {
            token.push_str("\r\r\n");
        }
    }
    token.push('}');
    token
}

pub fn format_1c_date_time(value: &str) -> Option<String> {
    if value.len() != 14 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}",
        &value[0..4],
        &value[4..6],
        &value[6..8],
        &value[8..10],
        &value[10..12],
        &value[12..14]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_base64_as_rfc_4648_does() {
        // RFC 4648 section 10 test vectors, then every byte value once.
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode_base64(input.as_bytes()), expected);
        }
        let every_byte: Vec<u8> = (0..=255).collect();
        let expected = concat!(
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4v",
            "MDEyMzQ1Njc4OTo7PD0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5f",
            "YGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn+AgYKDhIWGh4iJiouMjY6P",
            "kJGSk5SVlpeYmZqbnJ2en6ChoqOkpaanqKmqq6ytrq+wsbKztLW2t7i5uru8vb6/",
            "wMHCw8TFxsfIycrLzM3Oz9DR0tPU1dbX2Nna29zd3t/g4eLj5OXm5+jp6uvs7e7v",
            "8PHy8/T19vf4+fr7/P3+/w==",
        );
        assert_eq!(encode_base64(&every_byte), expected);
    }

    #[test]
    fn base64_value_reads_the_alphabet_and_nothing_else() {
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        for byte in 0..=u8::MAX {
            let expected = alphabet
                .iter()
                .position(|known| *known == byte)
                .map(|index| index as u8);
            assert_eq!(base64_value(byte), expected, "byte {byte}");
        }
    }

    #[test]
    fn decoding_reverses_encoding() {
        let bytes: Vec<u8> = (0..=255).rev().collect();
        for length in 0..=bytes.len() {
            let encoded = encode_base64(&bytes[..length]);
            assert_eq!(
                decode_base64_mime(&encoded).as_deref(),
                Some(&bytes[..length])
            );
        }
    }

    #[test]
    fn the_lenient_decoder_also_reads_a_group_padded_by_three_or_four() {
        for (input, lenient, strict) in [
            ("Q===", Some(vec![0x40]), None),
            ("====", Some(vec![0]), None),
            ("QUJD\r\nQ===", Some(b"ABC@".to_vec()), None),
            ("QQ==", Some(b"A".to_vec()), Some(b"A".to_vec())),
            ("QUI=", Some(b"AB".to_vec()), Some(b"AB".to_vec())),
            (
                "QUJD QUJD",
                Some(b"ABCABC".to_vec()),
                Some(b"ABCABC".to_vec()),
            ),
            ("Q=JD", None, None),
            ("QUJ", None, None),
            ("QU*D", None, None),
            ("", Some(vec![]), Some(vec![])),
        ] {
            assert_eq!(decode_base64_mime_lenient(input), lenient, "{input:?}");
            assert_eq!(decode_base64_mime(input), strict, "{input:?}");
        }
        let bytes: Vec<u8> = (0..=255).collect();
        for length in 0..=bytes.len() {
            let encoded = encode_base64(&bytes[..length]);
            assert_eq!(
                decode_base64_mime_lenient(&encoded).as_deref(),
                Some(&bytes[..length])
            );
        }
    }

    #[test]
    fn platform_token_breaks_every_full_line_of_64() {
        assert_eq!(platform_base64_token(b""), "{#base64:}");
        assert_eq!(platform_base64_token(b"foobar"), "{#base64:Zm9vYmFy}");

        // 48 bytes encode to exactly one full line, which still ends with
        // the break; 49 bytes spill four characters onto a second line.
        let line = encode_base64(&[7; 48]);
        assert_eq!(line.len(), 64);
        assert_eq!(
            platform_base64_token(&[7; 48]),
            format!("{{#base64:{line}\r\r\n}}")
        );
        let spilled = encode_base64(&[7; 49]);
        assert_eq!(
            platform_base64_token(&[7; 49]),
            format!("{{#base64:{}\r\r\n{}}}", &spilled[..64], &spilled[64..])
        );

        let bytes: Vec<u8> = (0..=255).collect();
        let token = platform_base64_token(&bytes);
        let payload = token
            .strip_prefix("{#base64:")
            .and_then(|rest| rest.strip_suffix('}'))
            .unwrap();
        let lines: Vec<&str> = payload.split("\r\r\n").collect();
        assert_eq!(lines.concat(), encode_base64(&bytes));
        assert!(lines[..lines.len() - 1].iter().all(|line| line.len() == 64));
        assert_eq!(lines.last().map(|line| line.len()), Some(344 % 64));
    }
}
