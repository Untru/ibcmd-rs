//! The platform's brace text as a plain tree.
//!
//! Every stored metadata descriptor row follows one layout rule, so a tree
//! needs no formatting flags: CRLF before every nested list's `{`, and CRLF
//! before a list's `}` when its last element is a list. The row starts with a
//! UTF-8 BOM.

use anyhow::{Result, anyhow, bail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Brace {
    /// `{a,b,...}`
    List(Vec<Brace>),
    /// A quoted string; holds the text without quotes or `""` escapes.
    Str(String),
    /// A bare token: number, uuid, `N`, ...
    Atom(String),
}

/// `list![a, b, ...]` builds a `Brace::List` from `Brace` values.
#[macro_export]
macro_rules! brace_list {
    ($($item:expr),* $(,)?) => {
        $crate::brace::Brace::List(vec![$($item),*])
    };
}

pub const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

impl Brace {
    pub fn list(items: Vec<Brace>) -> Self {
        Brace::List(items)
    }
    pub fn atom(value: impl ToString) -> Self {
        Brace::Atom(value.to_string())
    }
    pub fn str(value: impl Into<String>) -> Self {
        Brace::Str(value.into())
    }
    pub fn num(value: i64) -> Self {
        Brace::Atom(value.to_string())
    }
    /// `1` / `0`.
    pub fn flag(value: bool) -> Self {
        Brace::Atom(if value { "1" } else { "0" }.to_string())
    }
    pub fn uuid(value: &str) -> Self {
        Brace::Atom(value.to_ascii_lowercase())
    }
    pub fn nil_uuid() -> Self {
        Brace::Atom(NIL_UUID.to_string())
    }

    pub fn as_list(&self) -> Option<&[Brace]> {
        match self {
            Brace::List(items) => Some(items),
            _ => None,
        }
    }
    pub fn as_list_mut(&mut self) -> Option<&mut Vec<Brace>> {
        match self {
            Brace::List(items) => Some(items),
            _ => None,
        }
    }
    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Brace::Atom(value) => Some(value),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Brace::Str(value) => Some(value),
            _ => None,
        }
    }
    /// The element at a path of indexes (`&[1, 2]` = third element of the
    /// second element).
    pub fn at(&self, path: &[usize]) -> Option<&Brace> {
        let mut node = self;
        for index in path {
            node = node.as_list()?.get(*index)?;
        }
        Some(node)
    }
}

/// The stored row: BOM + text.
pub fn serialize_row(root: &Brace) -> Vec<u8> {
    let mut out = String::with_capacity(4096);
    out.push('\u{feff}');
    write_node(&mut out, root, false);
    out.into_bytes()
}

/// The text without BOM (for nested payloads and tests).
pub fn serialize(root: &Brace) -> String {
    let mut out = String::with_capacity(1024);
    write_node(&mut out, root, false);
    out
}

fn write_node(out: &mut String, node: &Brace, nested: bool) {
    match node {
        Brace::Atom(value) => out.push_str(value),
        Brace::Str(value) => {
            out.push('"');
            for ch in value.chars() {
                if ch == '"' {
                    out.push('"');
                }
                out.push(ch);
            }
            out.push('"');
        }
        Brace::List(items) => {
            if nested {
                out.push_str("\r\n");
            }
            out.push('{');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_node(out, item, true);
            }
            if matches!(items.last(), Some(Brace::List(_))) {
                out.push_str("\r\n");
            }
            out.push('}');
        }
    }
}

/// Parses a stored row (BOM optional).
pub fn parse_row(bytes: &[u8]) -> Result<Brace> {
    let text = std::str::from_utf8(bytes).map_err(|error| anyhow!("row is not UTF-8: {error}"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bytes = text.as_bytes();
    let mut position = skip_space(bytes, 0);
    let (root, end) = parse_list(text, position)?;
    position = skip_space(bytes, end);
    if position != bytes.len() {
        bail!("trailing text after the row at byte {position}");
    }
    Ok(root)
}

fn skip_space(bytes: &[u8], mut position: usize) -> usize {
    while position < bytes.len() && matches!(bytes[position], b'\r' | b'\n' | b' ' | b'\t') {
        position += 1;
    }
    position
}

fn parse_list(text: &str, start: usize) -> Result<(Brace, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        bail!("expected '{{' at byte {start}");
    }
    let mut items = Vec::new();
    let mut position = start + 1;
    let mut token_start: Option<usize> = None;
    loop {
        let Some(&byte) = bytes.get(position) else {
            bail!("unclosed list opened at byte {start}");
        };
        match byte {
            b'{' => {
                let (child, end) = parse_list(text, position)?;
                items.push(child);
                position = end;
            }
            b'"' => {
                let mut value = String::new();
                let mut cursor = position + 1;
                let mut segment = cursor;
                loop {
                    match bytes.get(cursor) {
                        None => bail!("unclosed string at byte {position}"),
                        Some(b'"') if bytes.get(cursor + 1) == Some(&b'"') => {
                            value.push_str(&text[segment..cursor + 1]);
                            cursor += 2;
                            segment = cursor;
                        }
                        Some(b'"') => {
                            value.push_str(&text[segment..cursor]);
                            break;
                        }
                        Some(_) => cursor += 1,
                    }
                }
                items.push(Brace::Str(value));
                position = cursor + 1;
            }
            b',' | b'}' => {
                if let Some(begin) = token_start.take() {
                    items.push(Brace::Atom(text[begin..position].trim().to_string()));
                }
                position += 1;
                if byte == b'}' {
                    return Ok((Brace::List(items), position));
                }
            }
            b'\r' | b'\n' => position += 1,
            _ => {
                if token_start.is_none() {
                    token_start = Some(position);
                }
                position += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_layout_rule() {
        let text = "\u{feff}{1,\r\n{16,\r\n{\"Pattern\",\r\n{\"S\",15,1}\r\n}\r\n},0,\"a\"\"b\",\r\n{0}\r\n}";
        let tree = parse_row(text.as_bytes()).unwrap();
        assert_eq!(String::from_utf8(serialize_row(&tree)).unwrap(), text);
        assert_eq!(tree.at(&[2]), Some(&Brace::Atom("0".into())));
        assert_eq!(tree.at(&[3]), Some(&Brace::Str("a\"b".into())));
    }
}
