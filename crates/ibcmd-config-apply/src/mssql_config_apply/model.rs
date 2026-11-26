//! Row metadata and small text helpers shared by the parts of the own apply.

use std::fmt::Write as _;

use serde::Serialize;

pub use ibcmd_restructure_engine::text::{hex_upper, is_uuid_text, quote_ident};

/// The metadata of one row of a configuration storage table (`Config`,
/// `ConfigSave`): everything but the bytes, which never leave the server
/// unless a check needs them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowMeta {
    pub name: String,
    pub part: i32,
    /// The `DataSize` column (the whole value; a multi-part row repeats it).
    pub data_size: i64,
    /// `DATALENGTH(BinaryData)`: the bytes this part holds.
    pub byte_len: i64,
    pub attributes: i16,
    pub creation: String,
    pub modified: String,
    /// Lower-case hex SHA-256 of `BinaryData`.
    pub sha256: String,
}

impl RowMeta {
    /// The identity of a row: SQL Server compares `FileName` case-insensitively
    /// (the column collation), so the key is lower-cased.
    pub fn key(&self) -> (String, i32) {
        (self.name.to_lowercase(), self.part)
    }
}

pub fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

/// What a `Config` row name is, by its shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowName<'a> {
    /// `root`, `version` or `versions`.
    Service(&'a str),
    /// `<uuid>`: a metadata object's descriptor.
    Descriptor(&'a str),
    /// `<uuid>.<suffix>`: a body row of the object (or nested object) `owner`.
    Body { owner: &'a str, suffix: &'a str },
    /// Anything else.
    Other,
}

pub fn classify_name(name: &str) -> RowName<'_> {
    if matches!(name, "root" | "version" | "versions") {
        return RowName::Service(name);
    }
    if is_uuid_text(name) {
        return RowName::Descriptor(name);
    }
    if name.len() > 37 && name.is_char_boundary(36) && name.as_bytes()[36] == b'.' {
        let (owner, suffix) = (&name[..36], &name[37..]);
        if is_uuid_text(owner)
            && !suffix.is_empty()
            && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return RowName::Body { owner, suffix };
        }
    }
    RowName::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_classified_by_shape() {
        let uuid = "ab132638-5188-470d-9432-de85f2b2c7d8";
        assert_eq!(classify_name("root"), RowName::Service("root"));
        assert_eq!(classify_name(uuid), RowName::Descriptor(uuid));
        assert_eq!(
            classify_name(&format!("{uuid}.0")),
            RowName::Body {
                owner: uuid,
                suffix: "0"
            }
        );
        assert_eq!(
            classify_name(&format!("{uuid}.1c")),
            RowName::Body {
                owner: uuid,
                suffix: "1c"
            }
        );
        assert_eq!(
            classify_name(&format!("{uuid}_dynupdate_{uuid}")),
            RowName::Other
        );
        assert_eq!(classify_name("DynamicallyUpdated"), RowName::Other);
        assert_eq!(classify_name(&format!("{uuid}.")), RowName::Other);
        assert_eq!(
            classify_name(&format!("{uuid}.new")),
            RowName::Body {
                owner: uuid,
                suffix: "new"
            }
        );
    }

    #[test]
    fn identifiers_are_quoted() {
        assert_eq!(quote_ident("a]b").unwrap(), "[a]]b]");
        assert!(quote_ident("").is_err());
        assert!(quote_ident("a\nb").is_err());
        assert_eq!(quote_string("it's"), "it''s");
    }
}
