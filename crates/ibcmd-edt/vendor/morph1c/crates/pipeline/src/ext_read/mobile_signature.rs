//! Complete Converted v2 signature framing. Only the four declared group counts
//! receive the EDT empty-list adaptation; member strings are never searched.

use std::ops::Range;

use formats_xml::registry::Format;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DigestMember {
    uuid: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConvertedSignature {
    public_key: String,
    digest: String,
    groups: [Vec<DigestMember>; 4],
    converted: bool,
}

pub(super) struct ConvertedSignatureFrame {
    model: ConvertedSignature,
    empty_counts: Vec<Range<usize>>,
    pub(super) native_compatible: bool,
}

impl ConvertedSignatureFrame {
    pub(super) fn decode(source: &[u8], format: Format) -> Result<Option<Self>, String> {
        let offset = if source.starts_with(b"\xef\xbb\xbf") {
            3
        } else {
            0
        };
        let Ok(text) = std::str::from_utf8(&source[offset..]) else {
            return Ok(None);
        };
        let mut cursor = Cursor { text, pos: 0 };
        if !cursor.eat(b'{') || cursor.atom().ok().map(|(a, _)| a) != Some("2") {
            return Ok(None);
        }
        // Legacy/unknown bodies keep their complete opaque contract. The known
        // v2 schema is selected only after its two strings and group container.
        if !cursor.eat(b',') || cursor.peek() != Some(b'"') {
            return Ok(None);
        }
        let public_key = cursor.quoted()?;
        if !cursor.eat(b',') || cursor.peek() != Some(b'"') {
            return Ok(None);
        }
        let digest = cursor.quoted()?;
        if !cursor.eat(b',') || !cursor.eat(b'{') {
            return Ok(None);
        }
        let mut groups = std::array::from_fn(|_| Vec::new());
        let mut empty_counts = Vec::new();
        let mut native_compatible = true;
        for (index, members) in groups.iter_mut().enumerate() {
            if index > 0 {
                cursor.expect(b',')?;
            }
            cursor.expect(b'{')?;
            let (atom, range) = cursor.atom()?;
            let count = atom
                .parse::<i32>()
                .map_err(|_| "invalid mobile digest group count")?;
            let count = match count {
                -1 if format == Format::Edt => {
                    native_compatible = false;
                    0
                }
                count if count >= 0 => count,
                _ => return Err("unsupported negative mobile digest group count".into()),
            };
            if count == 0 {
                empty_counts.push((range.start + offset)..(range.end + offset));
            }
            // Do not reserve using untrusted counts. Each actual member must
            // contain exactly the SDK's UUID and quoted name, in source order.
            for _ in 0..count {
                cursor.expect(b',')?;
                cursor.expect(b'{')?;
                let (uuid, _) = cursor.atom()?;
                if !valid_uuid(uuid) {
                    return Err("invalid mobile digest member UUID".into());
                }
                let uuid = uuid.to_ascii_lowercase();
                cursor.expect(b',')?;
                let name = cursor.quoted()?;
                cursor.expect(b'}')?;
                members.push(DigestMember { uuid, name });
            }
            cursor.expect(b'}')?;
        }
        cursor.expect(b'}')?;
        cursor.expect(b',')?;
        let converted = match cursor.atom()?.0 {
            "0" => false,
            "1" => true,
            _ => return Err("invalid mobile converted boolean".into()),
        };
        cursor.expect(b'}')?;
        if cursor.peek().is_some() {
            return Err("trailing mobile signature data".into());
        }
        Ok(Some(Self {
            model: ConvertedSignature {
                public_key,
                digest,
                groups,
                converted,
            },
            empty_counts,
            native_compatible,
        }))
    }

    pub(super) fn encode(&self, format: Format) -> Result<Vec<u8>, String> {
        let mut text = String::from("{2,");
        quote(&mut text, &self.model.public_key);
        text.push(',');
        quote(&mut text, &self.model.digest);
        text.push_str(",\n{\n");
        for (index, members) in self.model.groups.iter().enumerate() {
            if index > 0 {
                text.push_str(",\n");
            }
            text.push('{');
            if members.is_empty() && format == Format::Edt {
                text.push_str("-1");
            } else {
                text.push_str(&members.len().to_string());
            }
            for member in members {
                text.push_str(",\n{");
                text.push_str(&member.uuid);
                text.push(',');
                quote(&mut text, &member.name);
                text.push('}');
            }
            if !members.is_empty() {
                text.push('\n');
            }
            text.push('}');
        }
        text.push_str("\n},");
        text.push(if self.model.converted { '1' } else { '0' });
        text.push('}');
        Ok(text.into_bytes())
    }

    /// Preserve every source byte outside the four typed empty count atoms.
    pub(super) fn carrier_bytes(&self, source: &[u8]) -> Vec<u8> {
        if self.model.public_key.is_empty()
            && self.model.digest.is_empty()
            && self.model.groups.iter().all(Vec::is_empty)
        {
            return self
                .encode(Format::Edt)
                .expect("validated complete empty Converted model");
        }
        let mut output = source.to_vec();
        for range in self.empty_counts.iter().rev() {
            output.splice(range.clone(), b"-1".iter().copied());
        }
        output
    }
}

fn valid_uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn quote(output: &mut String, value: &str) {
    output.push('"');
    for c in value.chars() {
        output.push(c);
        if c == '"' {
            output.push('"');
        }
    }
    output.push('"');
}

struct Cursor<'a> {
    text: &'a str,
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn peek(&mut self) -> Option<u8> {
        while self
            .text
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.pos += 1;
        }
        self.text.as_bytes().get(self.pos).copied()
    }
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.eat(byte) {
            Ok(())
        } else {
            Err("invalid mobile signature list framing".into())
        }
    }
    fn atom(&mut self) -> Result<(&'a str, Range<usize>), String> {
        self.peek();
        let start = self.pos;
        while let Some(byte) = self.text.as_bytes().get(self.pos) {
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b'{' | b'}' | b'"') {
                break;
            }
            self.pos += 1;
        }
        if start == self.pos {
            return Err("missing mobile signature scalar".into());
        }
        Ok((&self.text[start..self.pos], start..self.pos))
    }
    fn quoted(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut output = String::new();
        let mut start = self.pos;
        loop {
            let Some(relative) = self.text[self.pos..].find('"') else {
                return Err("unclosed mobile signature string".into());
            };
            let end = self.pos + relative;
            output.push_str(&self.text[start..end]);
            self.pos = end + 1;
            if self.text.as_bytes().get(self.pos) == Some(&b'"') {
                output.push('"');
                self.pos += 1;
                start = self.pos;
            } else {
                return Ok(output);
            }
        }
    }
}
