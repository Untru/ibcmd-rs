//! Shared bounded codec for the textual native values used by bootstrap rows.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_cf::payload::{PayloadEncoding, decode_payload, encode_payload};
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::limits::ResourceLimits;

const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
const MAX_PLAIN_BYTES: usize = 64 * 1_048_576;
const MAX_NATIVE_DEPTH: usize = 64;
const MAX_NATIVE_NODES: usize = 500_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeValue {
    Token(String),
    Text(String),
    List {
        values: Vec<NativeValue>,
        leading_break: bool,
        line_breaks: Vec<usize>,
        trailing_break: bool,
    },
}

impl NativeValue {
    pub(crate) fn as_token(&self) -> Option<&str> {
        match self {
            Self::Token(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_list(&self) -> Option<&[NativeValue]> {
        match self {
            Self::List { values, .. } => Some(values),
            _ => None,
        }
    }
}

pub(crate) fn token(value: impl Into<String>) -> NativeValue {
    NativeValue::Token(value.into())
}

pub(crate) fn text(value: impl Into<String>) -> NativeValue {
    NativeValue::Text(value.into())
}

#[cfg(test)]
pub(crate) fn list(values: Vec<NativeValue>) -> NativeValue {
    let line_breaks = (1..values.len()).collect();
    NativeValue::List {
        values,
        leading_break: false,
        line_breaks,
        trailing_break: false,
    }
}

pub(crate) fn inline_list(values: Vec<NativeValue>) -> NativeValue {
    NativeValue::List {
        values,
        leading_break: false,
        line_breaks: Vec::new(),
        trailing_break: false,
    }
}

pub(crate) fn styled_list(values: Vec<NativeValue>, line_breaks: Vec<usize>) -> NativeValue {
    NativeValue::List {
        values,
        leading_break: false,
        line_breaks,
        trailing_break: false,
    }
}

pub(crate) fn styled_list_with_tail(
    values: Vec<NativeValue>,
    line_breaks: Vec<usize>,
) -> NativeValue {
    NativeValue::List {
        values,
        leading_break: false,
        line_breaks,
        trailing_break: true,
    }
}

pub(crate) fn formatted_list(
    values: Vec<NativeValue>,
    leading_break: bool,
    line_breaks: Vec<usize>,
    trailing_break: bool,
) -> NativeValue {
    NativeValue::List {
        values,
        leading_break,
        line_breaks,
        trailing_break,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeMetadataHeader {
    pub(crate) uuid: ObjectUuid,
    pub(crate) name: String,
    pub(crate) synonyms: Vec<(String, String)>,
    pub(crate) comment: String,
}

pub(crate) fn metadata_header(value: &NativeMetadataHeader) -> NativeValue {
    styled_list(
        vec![
            token("3"),
            inline_list(vec![token("1"), token("0"), token(value.uuid.to_string())]),
            text(&value.name),
            localized(&value.synonyms),
            text(&value.comment),
            token("0"),
            token("0"),
            token("00000000-0000-0000-0000-000000000000"),
            token("0"),
        ],
        vec![1, 3],
    )
}

pub(crate) fn localized(values: &[(String, String)]) -> NativeValue {
    let mut fields = Vec::with_capacity(values.len() * 2 + 1);
    fields.push(token(values.len().to_string()));
    for (language, content) in values {
        fields.push(text(language));
        fields.push(text(content));
    }
    inline_list(fields)
}

pub(crate) fn parse_metadata_header(
    value: &NativeValue,
) -> Result<NativeMetadataHeader, NativeError> {
    let fields = exact_list(value, 9, "metadata header")?;
    exact_token(&fields[0], "3", "metadata header marker")?;
    let identity = exact_list(&fields[1], 3, "metadata identity")?;
    exact_token(&identity[0], "1", "metadata identity marker")?;
    exact_token(&identity[1], "0", "metadata identity kind")?;
    let uuid_text = required_token(&identity[2], "metadata UUID")?;
    let uuid = ObjectUuid::parse(uuid_text).map_err(|_| NativeError::Shape {
        field: "metadata UUID",
    })?;
    for (index, expected) in [(5, "0"), (6, "0"), (8, "0")] {
        exact_token(&fields[index], expected, "metadata header reserved field")?;
    }
    exact_token(
        &fields[7],
        "00000000-0000-0000-0000-000000000000",
        "metadata header reserved UUID",
    )?;
    Ok(NativeMetadataHeader {
        uuid,
        name: required_text(&fields[2], "metadata name")?.to_owned(),
        synonyms: parse_localized(&fields[3])?,
        comment: required_text(&fields[4], "metadata comment")?.to_owned(),
    })
}

pub(crate) fn parse_localized(value: &NativeValue) -> Result<Vec<(String, String)>, NativeError> {
    let fields = required_list(value, "localized value")?;
    let count = required_token(
        fields.first().ok_or(NativeError::Shape {
            field: "localized count",
        })?,
        "localized count",
    )?
    .parse::<usize>()
    .map_err(|_| NativeError::Shape {
        field: "localized count",
    })?;
    let expected = count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(NativeError::NodeOverflow)?;
    if fields.len() != expected {
        return Err(NativeError::Shape {
            field: "localized item count",
        });
    }
    let mut values = Vec::with_capacity(count);
    for pair in fields[1..].chunks_exact(2) {
        values.push((
            required_text(&pair[0], "localized language")?.to_owned(),
            required_text(&pair[1], "localized content")?.to_owned(),
        ));
    }
    Ok(values)
}

pub(crate) fn required_list<'a>(
    value: &'a NativeValue,
    field: &'static str,
) -> Result<&'a [NativeValue], NativeError> {
    value.as_list().ok_or(NativeError::Shape { field })
}

pub(crate) fn exact_list<'a>(
    value: &'a NativeValue,
    length: usize,
    field: &'static str,
) -> Result<&'a [NativeValue], NativeError> {
    let values = required_list(value, field)?;
    if values.len() != length {
        return Err(NativeError::Shape { field });
    }
    Ok(values)
}

pub(crate) fn required_token<'a>(
    value: &'a NativeValue,
    field: &'static str,
) -> Result<&'a str, NativeError> {
    value.as_token().ok_or(NativeError::Shape { field })
}

pub(crate) fn required_text<'a>(
    value: &'a NativeValue,
    field: &'static str,
) -> Result<&'a str, NativeError> {
    value.as_text().ok_or(NativeError::Shape { field })
}

pub(crate) fn exact_token(
    value: &NativeValue,
    expected: &str,
    field: &'static str,
) -> Result<(), NativeError> {
    if required_token(value, field)? != expected {
        return Err(NativeError::Shape { field });
    }
    Ok(())
}

pub(crate) fn parse_bool_token(
    value: &NativeValue,
    field: &'static str,
) -> Result<bool, NativeError> {
    match required_token(value, field)? {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(NativeError::Shape { field }),
    }
}

pub(crate) fn serialize(value: &NativeValue) -> Result<Vec<u8>, NativeError> {
    let mut output = Vec::new();
    output.extend_from_slice(UTF8_BOM);
    write_value(value, &mut output, 0)?;
    Ok(output)
}

pub(crate) fn serialize_without_bom(value: &NativeValue) -> Result<Vec<u8>, NativeError> {
    let bytes = serialize(value)?;
    Ok(bytes[UTF8_BOM.len()..].to_vec())
}

pub(crate) fn parse(input: &[u8]) -> Result<NativeValue, NativeError> {
    NativeParser::new(input).parse()
}

pub(crate) fn parse_without_bom(input: &[u8]) -> Result<NativeValue, NativeError> {
    let total =
        input
            .len()
            .checked_add(UTF8_BOM.len())
            .ok_or(NativeError::PlainPayloadTooLarge {
                maximum: MAX_PLAIN_BYTES,
                actual: usize::MAX,
            })?;
    if total > MAX_PLAIN_BYTES {
        return Err(NativeError::PlainPayloadTooLarge {
            maximum: MAX_PLAIN_BYTES,
            actual: total,
        });
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(UTF8_BOM);
    bytes.extend_from_slice(input);
    parse(&bytes)
}

pub(crate) fn raw_deflate(value: &NativeValue) -> Result<Vec<u8>, NativeError> {
    let plain = serialize(value)?;
    encode_payload(PayloadEncoding::RawDeflate, &plain, payload_limits())
        .map_err(|error| NativeError::Payload(error.to_string()))
}

pub(crate) fn inflate_and_parse(blob: &[u8]) -> Result<NativeValue, NativeError> {
    let plain = decode_payload(PayloadEncoding::RawDeflate, blob, payload_limits())
        .map_err(|error| NativeError::Payload(error.to_string()))?;
    parse(plain.bytes())
}

pub(crate) fn inflate(blob: &[u8]) -> Result<Vec<u8>, NativeError> {
    decode_payload(PayloadEncoding::RawDeflate, blob, payload_limits())
        .map(|payload| payload.into_bytes())
        .map_err(|error| NativeError::Payload(error.to_string()))
}

pub(crate) fn deflate_bytes(bytes: &[u8]) -> Result<Vec<u8>, NativeError> {
    encode_payload(PayloadEncoding::RawDeflate, bytes, payload_limits())
        .map_err(|error| NativeError::Payload(error.to_string()))
}

fn payload_limits() -> ResourceLimits {
    ResourceLimits::new(
        MAX_NATIVE_DEPTH,
        1,
        MAX_PLAIN_BYTES as u64,
        MAX_PLAIN_BYTES as u64,
        200,
    )
    .expect("native payload limits are non-zero")
}

fn write_value(value: &NativeValue, output: &mut Vec<u8>, depth: usize) -> Result<(), NativeError> {
    if depth > MAX_NATIVE_DEPTH {
        return Err(NativeError::DepthExceeded);
    }
    match value {
        NativeValue::Token(value) => {
            if is_line_wrapped_base64_token(value.as_bytes()) {
                output.extend_from_slice(value.as_bytes());
            } else if value.is_empty()
                || !value.is_ascii()
                || value.bytes().any(|byte| {
                    byte.is_ascii_whitespace() || matches!(byte, b'{' | b'}' | b',' | b'"')
                })
            {
                return Err(NativeError::InvalidToken);
            } else {
                output.extend_from_slice(value.as_bytes());
            }
        }
        NativeValue::Text(value) => {
            output.push(b'"');
            for byte in value.as_bytes() {
                output.push(*byte);
                if *byte == b'"' {
                    output.push(b'"');
                }
            }
            output.push(b'"');
        }
        NativeValue::List {
            values,
            leading_break,
            line_breaks,
            trailing_break,
        } => {
            output.push(b'{');
            if *leading_break && !values.is_empty() {
                output.extend_from_slice(b"\r\n");
            }
            for (index, child) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                    if line_breaks.binary_search(&index).is_ok() {
                        output.extend_from_slice(b"\r\n");
                    }
                }
                write_value(child, output, depth + 1)?;
            }
            if *trailing_break && !values.is_empty() {
                output.extend_from_slice(b"\r\n");
            }
            output.push(b'}');
        }
    }
    if output.len() > MAX_PLAIN_BYTES {
        return Err(NativeError::PlainPayloadTooLarge {
            maximum: MAX_PLAIN_BYTES,
            actual: output.len(),
        });
    }
    Ok(())
}

struct NativeParser<'a> {
    input: &'a [u8],
    offset: usize,
    nodes: usize,
}

impl<'a> NativeParser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            offset: 0,
            nodes: 0,
        }
    }

    fn parse(mut self) -> Result<NativeValue, NativeError> {
        if self.input.len() > MAX_PLAIN_BYTES {
            return Err(NativeError::PlainPayloadTooLarge {
                maximum: MAX_PLAIN_BYTES,
                actual: self.input.len(),
            });
        }
        if !self.input.starts_with(UTF8_BOM) {
            return Err(NativeError::MissingBom);
        }
        self.offset = UTF8_BOM.len();
        let value = self.value(0)?;
        self.whitespace();
        if self.offset != self.input.len() {
            return Err(NativeError::TrailingBytes);
        }
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<NativeValue, NativeError> {
        if depth > MAX_NATIVE_DEPTH {
            return Err(NativeError::DepthExceeded);
        }
        self.nodes = self.nodes.checked_add(1).ok_or(NativeError::NodeOverflow)?;
        if self.nodes > MAX_NATIVE_NODES {
            return Err(NativeError::TooManyNodes);
        }
        self.whitespace();
        match self.input.get(self.offset) {
            Some(b'{') => self.list(depth),
            Some(b'"') => self.text(),
            Some(_) => self.token(),
            None => Err(NativeError::UnexpectedEnd),
        }
    }

    fn list(&mut self, depth: usize) -> Result<NativeValue, NativeError> {
        self.offset += 1;
        let whitespace_start = self.offset;
        self.whitespace();
        let leading_break = self.input[whitespace_start..self.offset]
            .iter()
            .any(|byte| matches!(byte, b'\r' | b'\n'));
        let mut values = Vec::new();
        if self.input.get(self.offset) == Some(&b'}') {
            self.offset += 1;
            return Ok(NativeValue::List {
                values,
                leading_break: false,
                line_breaks: Vec::new(),
                trailing_break: false,
            });
        }
        let mut line_breaks = Vec::new();
        loop {
            values.push(self.value(depth + 1)?);
            let whitespace_start = self.offset;
            self.whitespace();
            let trailing_break = self.input[whitespace_start..self.offset]
                .iter()
                .any(|byte| matches!(byte, b'\r' | b'\n'));
            match self.input.get(self.offset) {
                Some(b',') => {
                    self.offset += 1;
                    let whitespace_start = self.offset;
                    self.whitespace();
                    if self.input[whitespace_start..self.offset]
                        .iter()
                        .any(|byte| matches!(byte, b'\r' | b'\n'))
                    {
                        line_breaks.push(values.len());
                    }
                    if self.input.get(self.offset) == Some(&b'}') {
                        return Err(NativeError::TrailingComma);
                    }
                }
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(NativeValue::List {
                        values,
                        leading_break,
                        line_breaks,
                        trailing_break,
                    });
                }
                Some(_) => return Err(NativeError::ExpectedDelimiter),
                None => return Err(NativeError::UnexpectedEnd),
            }
        }
    }

    fn text(&mut self) -> Result<NativeValue, NativeError> {
        self.offset += 1;
        let mut output = Vec::new();
        loop {
            let Some(byte) = self.input.get(self.offset).copied() else {
                return Err(NativeError::UnexpectedEnd);
            };
            self.offset += 1;
            if byte != b'"' {
                output.push(byte);
                continue;
            }
            if self.input.get(self.offset) == Some(&b'"') {
                output.push(b'"');
                self.offset += 1;
                continue;
            }
            return String::from_utf8(output)
                .map(NativeValue::Text)
                .map_err(|_| NativeError::InvalidUtf8);
        }
    }

    fn token(&mut self) -> Result<NativeValue, NativeError> {
        if self.input[self.offset..].starts_with(b"#base64:") {
            return self.line_wrapped_base64_token();
        }
        let start = self.offset;
        while let Some(byte) = self.input.get(self.offset) {
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b'}') {
                break;
            }
            if matches!(byte, b'{' | b'"') || !byte.is_ascii() {
                return Err(NativeError::InvalidToken);
            }
            self.offset += 1;
        }
        if self.offset == start {
            return Err(NativeError::InvalidToken);
        }
        let value = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| NativeError::InvalidUtf8)?;
        Ok(NativeValue::Token(value.to_owned()))
    }

    fn line_wrapped_base64_token(&mut self) -> Result<NativeValue, NativeError> {
        let start = self.offset;
        self.offset += b"#base64:".len();
        let mut payload_len = 0usize;
        let mut padding_len = 0usize;
        while let Some(byte) = self.input.get(self.offset).copied() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' if padding_len == 0 => {
                    payload_len += 1;
                    self.offset += 1;
                }
                b'=' => {
                    padding_len += 1;
                    if padding_len > 2 {
                        return Err(NativeError::InvalidToken);
                    }
                    self.offset += 1;
                }
                _ if byte.is_ascii_whitespace() => self.offset += 1,
                b',' | b'}' => break,
                _ => return Err(NativeError::InvalidToken),
            }
        }
        if payload_len == 0 || (payload_len + padding_len) % 4 != 0 {
            return Err(NativeError::InvalidToken);
        }
        let value = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| NativeError::InvalidUtf8)?;
        Ok(NativeValue::Token(value.to_owned()))
    }

    fn whitespace(&mut self) {
        while self
            .input
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
}

fn is_line_wrapped_base64_token(value: &[u8]) -> bool {
    let Some(payload) = value.strip_prefix(b"#base64:") else {
        return false;
    };
    let mut payload_len = 0usize;
    let mut padding_len = 0usize;
    for byte in payload {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' if padding_len == 0 => {
                payload_len += 1;
            }
            b'=' => {
                padding_len += 1;
                if padding_len > 2 {
                    return false;
                }
            }
            _ if byte.is_ascii_whitespace() => {}
            _ => return false,
        }
    }
    payload_len > 0 && (payload_len + padding_len) % 4 == 0
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeError {
    MissingBom,
    UnexpectedEnd,
    TrailingBytes,
    TrailingComma,
    ExpectedDelimiter,
    InvalidToken,
    InvalidUtf8,
    DepthExceeded,
    NodeOverflow,
    TooManyNodes,
    PlainPayloadTooLarge { maximum: usize, actual: usize },
    Payload(String),
    Shape { field: &'static str },
}

impl Display for NativeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBom => formatter.write_str("native value has no UTF-8 BOM"),
            Self::UnexpectedEnd => formatter.write_str("native value ended unexpectedly"),
            Self::TrailingBytes => formatter.write_str("native value has trailing bytes"),
            Self::TrailingComma => formatter.write_str("native list has a trailing comma"),
            Self::ExpectedDelimiter => {
                formatter.write_str("native list expected comma or closing brace")
            }
            Self::InvalidToken => formatter.write_str("native token contains a reserved byte"),
            Self::InvalidUtf8 => formatter.write_str("native text is not valid UTF-8"),
            Self::DepthExceeded => formatter.write_str("native value exceeds its depth bound"),
            Self::NodeOverflow => formatter.write_str("native node counter overflow"),
            Self::TooManyNodes => formatter.write_str("native value exceeds its node bound"),
            Self::PlainPayloadTooLarge { maximum, actual } => write!(
                formatter,
                "native plaintext has {actual} bytes, exceeding {maximum}"
            ),
            Self::Payload(reason) => {
                write!(formatter, "native payload codec rejected data: {reason}")
            }
            Self::Shape { field } => write!(formatter, "native value has invalid {field} shape"),
        }
    }
}

impl Error for NativeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textual_value_and_raw_deflate_roundtrip() {
        let value = list(vec![
            token("1"),
            text("Текст \"в кавычках\""),
            list(vec![token("0")]),
        ]);
        let plain = serialize(&value).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert!(plain.windows(2).any(|bytes| bytes == b"\r\n"));
        assert_eq!(parse(&plain).unwrap(), value);
        assert_eq!(
            inflate_and_parse(&raw_deflate(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn parser_rejects_missing_bom_and_trailing_data() {
        assert_eq!(parse(b"{1}"), Err(NativeError::MissingBom));
        let mut input = UTF8_BOM.to_vec();
        input.extend_from_slice(b"{1}x");
        assert_eq!(parse(&input), Err(NativeError::TrailingBytes));
    }

    #[test]
    fn parser_preserves_line_wrapped_base64_atom() {
        let input = b"\xef\xbb\xbf{#base64:QUJD\r\nRA==}";
        let value = parse(input).unwrap();
        assert_eq!(
            value,
            NativeValue::List {
                values: vec![NativeValue::Token("#base64:QUJD\r\nRA==".to_string())],
                leading_break: false,
                line_breaks: Vec::new(),
                trailing_break: false,
            }
        );
        assert_eq!(serialize(&value).unwrap(), input);
    }

    #[test]
    fn parser_rejects_malformed_line_wrapped_base64_and_generic_wrapped_tokens() {
        for input in [
            b"\xef\xbb\xbf{#base64:QU?D}".as_slice(),
            b"\xef\xbb\xbf{#base64:QUJD=}".as_slice(),
            b"\xef\xbb\xbf{#base64:QUJD=AAA}".as_slice(),
            b"\xef\xbb\xbf{ordinary\r\ntoken}".as_slice(),
        ] {
            assert!(matches!(
                parse(input),
                Err(NativeError::InvalidToken | NativeError::ExpectedDelimiter)
            ));
        }
    }
}
