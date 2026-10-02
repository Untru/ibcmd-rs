//! JSON-RPC 2.0 over a byte stream in `Content-Length` frames, as the
//! Language Server Protocol frames it: a header block of `Name: value` lines
//! ended by an empty line (`\r\n\r\n`), then exactly `Content-Length` bytes
//! of UTF-8 JSON. Only `Content-Length` is read; `Content-Type` and any other
//! header is accepted and ignored.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

/// The largest message body read (a request is a few hundred bytes; a
/// `source/read` answer is written, not read).
pub const MAX_BODY_BYTES: usize = 64 << 20;

/// JSON-RPC 2.0 and LSP error codes, and the server's own (-32010..-32019,
/// inside the range JSON-RPC leaves to implementations).
pub mod codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    /// LSP: a request before `initialize`.
    pub const SERVER_NOT_INITIALIZED: i64 = -32002;
    /// LSP: the request was cancelled (`$/cancelRequest`).
    pub const REQUEST_CANCELLED: i64 = -32800;
    /// `initialize`: the client speaks a protocol version this server does not.
    pub const INCOMPATIBLE_PROTOCOL: i64 = -32010;
    /// No infobase of that id.
    pub const UNKNOWN_INFOBASE: i64 = -32011;
    /// The configuration holds no object of that name.
    pub const UNKNOWN_OBJECT: i64 = -32012;
    /// The operation needs SQL Server and the infobase is a rows folder.
    pub const NOT_SUPPORTED_OFFLINE: i64 = -32013;
    /// The operation ran and failed (SQL Server, the export, the disk).
    pub const OPERATION_FAILED: i64 = -32014;
    /// A write into the database was asked for without `confirm: true`.
    pub const CONFIRMATION_REQUIRED: i64 = -32015;
}

/// An error answer: code, message and optional data.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_PARAMS, message)
    }

    pub fn to_json(&self) -> Value {
        let mut error = json!({"code": self.code, "message": self.message});
        if let Some(data) = &self.data {
            error["data"] = data.clone();
        }
        error
    }
}

/// Reads one frame's body; `None` at the end of the stream before a frame
/// begins. A malformed header block is an error: the stream cannot be
/// resynchronised after it.
pub fn read_frame(reader: &mut (impl BufRead + ?Sized)) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    let mut started = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            if started {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the stream ended inside a frame header",
                ));
            }
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if !started {
                // Blank lines between frames are tolerated.
                continue;
            }
            break;
        }
        started = true;
        let Some((name, value)) = line.split_once(':') else {
            return Err(invalid(format!("malformed frame header line {line:?}")));
        };
        if name.trim().eq_ignore_ascii_case("Content-Length") {
            let value = value.trim().parse::<usize>().map_err(|_| {
                invalid(format!("Content-Length {:?} is not a number", value.trim()))
            })?;
            length = Some(value);
        }
    }
    let length =
        length.ok_or_else(|| invalid("a frame header has no Content-Length".to_string()))?;
    if length > MAX_BODY_BYTES {
        return Err(invalid(format!(
            "a frame of {length} bytes exceeds the {MAX_BODY_BYTES}-byte limit"
        )));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Writes one frame and flushes it.
pub fn write_frame(writer: &mut dyn Write, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(io::Error::other)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// A response to `id`: the result, or the error.
pub fn response(id: &Value, outcome: Result<Value, RpcError>) -> Value {
    match outcome {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error.to_json()}),
    }
}

/// A notification: a method and its params, no id.
pub fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

/// Standard base64 (RFC 4648, with padding) of a file the editor gets as
/// bytes.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(ALPHABET[((triple >> (18 - 6 * index)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_ignore_other_headers() {
        let mut stream = Vec::new();
        write_frame(&mut stream, &json!({"jsonrpc": "2.0", "method": "x"})).unwrap();
        let text = String::from_utf8(stream.clone()).unwrap();
        assert!(text.starts_with("Content-Length: "));
        let mut with_type = b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n".to_vec();
        with_type.extend(&stream);
        for input in [stream, with_type] {
            let mut reader = io::Cursor::new(input);
            let body = read_frame(&mut reader).unwrap().unwrap();
            let value: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["method"], "x");
            assert!(read_frame(&mut reader).unwrap().is_none());
        }
    }

    #[test]
    fn a_frame_counts_bytes_not_characters() {
        let body = "{\"п\":\"ё\"}";
        let input = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let mut reader = io::Cursor::new(input.into_bytes());
        assert_eq!(read_frame(&mut reader).unwrap().unwrap(), body.as_bytes());
    }

    #[test]
    fn a_header_block_without_length_is_refused() {
        let mut reader = io::Cursor::new(b"Content-Type: x\r\n\r\n{}".to_vec());
        assert!(read_frame(&mut reader).is_err());
    }

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }
}
