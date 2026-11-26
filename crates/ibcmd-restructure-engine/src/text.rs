//! Text primitives shared by native apply and restructuring.

use std::fmt::Write as _;

/// `[name]` for a database, refusing what cannot be one.
pub fn quote_ident(value: &str) -> anyhow::Result<String> {
    if value.is_empty()
        || value.encode_utf16().count() > 128
        || value.chars().any(|ch| ch == '\0' || ch.is_control())
    {
        anyhow::bail!("invalid database identifier {value:?}");
    }
    Ok(format!("[{}]", value.replace(']', "]]")))
}

pub fn hex_upper(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02X}").expect("writing to a String cannot fail");
    }
    out
}

/// `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` (any case).
pub fn is_uuid_text(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}
