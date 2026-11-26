//! The braced-record text primitives `src/module_blob.rs` reads and patches
//! stored records with, moved from there unchanged but for one mechanical
//! clippy fix (`manual_strip` in `decode_1c_string_body`); the root reaches
//! every item by its old name through imports. They read the end and the
//! value of a 1C string literal, the end of a braced value, the byte ranges
//! of its top-level fields and a `{#base64:...}` payload field.
//!
//! They are not `ibcmd_1c_text`'s splitter and string reader, which the form
//! body reader uses, and do not behave alike: these answer an error rather
//! than `None`; the field splitter returns byte ranges trimmed of ASCII
//! whitespace only and reads no per-thread jump table; the string reader
//! refuses anything after the closing quote.

use std::ops::Range;

use anyhow::{Result, anyhow};
use ibcmd_1c_text::decode_base64_mime_lenient;

pub fn decode_base64_payload_field(field: &str) -> Result<Vec<u8>> {
    let Some(payload) = field
        .strip_prefix("{#base64:")
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Err(anyhow!("base64 payload wrapper is missing"));
    };
    // `module_blob`'s decoder (`ibcmd_1c_text::decode_base64_mime_lenient`),
    // so a payload field decodes exactly as it did in the root.
    decode_base64_mime_lenient(payload).ok_or_else(|| anyhow!("base64 payload is invalid"))
}

pub fn scan_1c_quoted_string_end(text: &str, start: usize) -> Result<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return Err(anyhow!("expected 1C string at byte {start}"));
    }
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            if bytes.get(index + 1) == Some(&b'"') {
                index += 2;
            } else if is_1c_string_escape_unit(bytes, index + 1) {
                // A quote, a backslash and four lowercase hex digits spell one
                // UTF-16 unit of a character outside the basic plane; the
                // literal goes on behind them.
                index += 6;
            } else {
                return Ok(index + 1);
            }
        } else {
            index += 1;
        }
    }
    Err(anyhow!("unterminated 1C string at byte {start}"))
}

/// Whether `bytes[at..]` starts with `\` and four lowercase hex digits (the
/// rest of a `"\d83d`-style code unit escape inside a 1C string literal).
fn is_1c_string_escape_unit(bytes: &[u8], at: usize) -> bool {
    bytes.get(at) == Some(&b'\\')
        && bytes.get(at + 1..at + 5).is_some_and(|digits| {
            digits
                .iter()
                .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(digit))
        })
}

/// The characters of the inside of a 1C string literal: `""` is a quote and
/// `"\xxxx` one UTF-16 code unit, a high one waiting for its low partner.
fn decode_1c_string_body(inner: &str) -> Result<String> {
    let mut output = String::with_capacity(inner.len());
    let mut pending_high: Option<u16> = None;
    let mut rest = inner;
    while let Some(character) = rest.chars().next() {
        if character != '"' {
            if pending_high.is_some() {
                return Err(anyhow!("unpaired high surrogate in a 1C string"));
            }
            output.push(character);
            rest = &rest[character.len_utf8()..];
            continue;
        }
        let tail = &rest[1..];
        if let Some(after_quote) = tail.strip_prefix('"') {
            if pending_high.is_some() {
                return Err(anyhow!("unpaired high surrogate in a 1C string"));
            }
            output.push('"');
            rest = after_quote;
            continue;
        }
        if is_1c_string_escape_unit(tail.as_bytes(), 0) {
            let unit = u16::from_str_radix(&tail[1..5], 16)
                .map_err(|_| anyhow!("bad code unit escape in a 1C string"))?;
            match unit {
                0xD800..=0xDBFF => {
                    if pending_high.is_some() {
                        return Err(anyhow!("unpaired high surrogate in a 1C string"));
                    }
                    pending_high = Some(unit);
                }
                0xDC00..=0xDFFF => {
                    let high = u32::from(
                        pending_high
                            .take()
                            .ok_or_else(|| anyhow!("unpaired low surrogate in a 1C string"))?,
                    );
                    let scalar = 0x10000 + ((high - 0xD800) << 10) + (u32::from(unit) - 0xDC00);
                    output.push(
                        char::from_u32(scalar)
                            .ok_or_else(|| anyhow!("bad surrogate pair in a 1C string"))?,
                    );
                }
                _ => {
                    if pending_high.is_some() {
                        return Err(anyhow!("unpaired high surrogate in a 1C string"));
                    }
                    output.push(
                        char::from_u32(u32::from(unit))
                            .ok_or_else(|| anyhow!("bad code unit in a 1C string"))?,
                    );
                }
            }
            rest = &tail[5..];
            continue;
        }
        return Err(anyhow!("unexpected quote inside a 1C string"));
    }
    if pending_high.is_some() {
        return Err(anyhow!("unpaired high surrogate in a 1C string"));
    }
    Ok(output)
}

pub fn parse_1c_quoted_string(text: &str) -> Result<String> {
    let text = text.trim();
    let end = scan_1c_quoted_string_end(text, 0)?;
    if end != text.len() {
        return Err(anyhow!("unexpected trailing data after 1C string"));
    }
    decode_1c_string_body(&text[1..end - 1])
}

pub fn scan_balanced_braces(text: &str, start: usize) -> Result<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return Err(anyhow!("expected 1C braced expression at byte {start}"));
    }

    let mut index = start;
    let mut depth = 0usize;
    let mut in_string = false;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if in_string && bytes.get(index + 1) == Some(&b'"') => {
                index += 2;
            }
            b'"' => {
                in_string = !in_string;
                index += 1;
            }
            b'{' if !in_string => {
                depth += 1;
                index += 1;
            }
            b'}' if !in_string => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| anyhow!("unbalanced 1C braced expression"))?;
                index += 1;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    Err(anyhow!("unterminated 1C braced expression at byte {start}"))
}

pub fn scan_braced_fields(text: &str, start: usize) -> Result<Vec<Range<usize>>> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return Err(anyhow!("expected 1C braced expression at byte {start}"));
    }

    let mut fields = Vec::new();
    let mut field_start = start + 1;
    let mut index = start + 1;
    let mut depth = 0usize;
    let mut in_string = false;

    while index < bytes.len() {
        match bytes[index] {
            b'"' if in_string && bytes.get(index + 1) == Some(&b'"') => {
                index += 2;
            }
            b'"' => {
                in_string = !in_string;
                index += 1;
            }
            b'{' if !in_string => {
                depth += 1;
                index += 1;
            }
            b'}' if !in_string && depth > 0 => {
                depth -= 1;
                index += 1;
            }
            b'}' if !in_string => {
                fields.push(trim_ascii_ws_range(text, field_start..index));
                return Ok(fields);
            }
            b',' if !in_string && depth == 0 => {
                fields.push(trim_ascii_ws_range(text, field_start..index));
                field_start = index + 1;
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    Err(anyhow!("unterminated 1C braced expression at byte {start}"))
}

pub fn trim_ascii_ws_range(text: &str, range: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let mut start = range.start;
    let mut end = range.end;
    while start < end
        && bytes
            .get(start)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        start += 1;
    }
    while end > start
        && bytes
            .get(end - 1)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        end -= 1;
    }
    start..end
}

#[cfg(test)]
mod tests {
    /// A character outside the basic plane is written inside a string literal
    /// as two code unit escapes, each behind a quote (`"\d83d"\dcce`): the
    /// literal goes on behind them, and neither the scan for the end of the
    /// literal nor the decoding may stop at the quote.
    #[test]
    fn a_string_literal_holds_code_unit_escapes() {
        let literal = "\"a>\"\\d83d\"\\dcce \"\" b\"";
        assert_eq!(
            super::scan_1c_quoted_string_end(literal, 0).unwrap(),
            literal.len()
        );
        assert_eq!(
            super::parse_1c_quoted_string(literal).unwrap(),
            "a>\u{1F4CE} \" b"
        );
        // Lone halves are refused, not replaced.
        assert!(super::parse_1c_quoted_string("\"\"\\d83d\"").is_err());
        assert!(super::parse_1c_quoted_string("\"\"\\dcce\"").is_err());
        // A closing quote followed by text that merely looks like an escape
        // (uppercase digits, a short run) still closes the literal.
        assert_eq!(
            super::scan_1c_quoted_string_end("\"x\",\\D83D", 0).unwrap(),
            3
        );
    }
}
