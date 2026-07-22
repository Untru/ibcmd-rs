//! Container revision detection and unified structural parse entry point.

use std::{error::Error, fmt};

use crate::{format15, format16};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Revision {
    Format15,
    Format16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Detection {
    pub revision: Revision,
    pub base_offset: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Container {
    Format15(format15::Container),
    Format16(format16::Container),
}

impl Container {
    #[must_use]
    pub fn revision(&self) -> Revision {
        match self {
            Self::Format15(_) => Revision::Format15,
            Self::Format16(_) => Revision::Format16,
        }
    }

    #[must_use]
    pub fn base_offset(&self) -> usize {
        match self {
            Self::Format15(_) => 0,
            Self::Format16(container) => container.base_offset,
        }
    }

    #[must_use]
    pub fn element_count(&self) -> usize {
        match self {
            Self::Format15(container) => container.elements.len(),
            Self::Format16(container) => container.elements.len(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DetectError {
    InputTooShort { minimum: usize, actual: usize },
    UnknownSignature { found: u32 },
}

impl fmt::Display for DetectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooShort { minimum, actual } => write!(
                formatter,
                "container is too short for revision detection: expected at least {minimum} bytes, got {actual}"
            ),
            Self::UnknownSignature { found } => write!(
                formatter,
                "unknown V8 container signature: first 32-bit word is 0x{found:08x}"
            ),
        }
    }
}

impl Error for DetectError {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ParseError {
    Detect(DetectError),
    Format15(format15::Format15Error),
    Format16(format16::Format16Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Detect(error) => error.fmt(formatter),
            Self::Format15(error) => error.fmt(formatter),
            Self::Format16(error) => error.fmt(formatter),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Detect(error) => Some(error),
            Self::Format15(error) => Some(error),
            Self::Format16(error) => Some(error),
        }
    }
}

impl From<DetectError> for ParseError {
    fn from(error: DetectError) -> Self {
        Self::Detect(error)
    }
}

/// Detect the revision using the two independent signatures used by the
/// format: every complete artifact starts with a Format15 envelope, while a
/// Format16 artifact additionally has its 64-bit sentinel at `0x1359`.
pub fn detect(bytes: &[u8]) -> Result<Detection, DetectError> {
    if bytes.len() < 4 {
        return Err(DetectError::InputTooShort {
            minimum: 4,
            actual: bytes.len(),
        });
    }
    let first = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    if first != format15::SENTINEL {
        return Err(DetectError::UnknownSignature { found: first });
    }

    let wide_end = format16::BASE_OFFSET + 8;
    if bytes.len() >= wide_end {
        let marker = u64::from_le_bytes(bytes[format16::BASE_OFFSET..wide_end].try_into().unwrap());
        if marker == format16::SENTINEL {
            return Ok(Detection {
                revision: Revision::Format16,
                base_offset: format16::BASE_OFFSET,
            });
        }
    }

    Ok(Detection {
        revision: Revision::Format15,
        base_offset: 0,
    })
}

pub fn parse(bytes: &[u8]) -> Result<Container, ParseError> {
    match detect(bytes)?.revision {
        Revision::Format15 => format15::parse(bytes)
            .map(Container::Format15)
            .map_err(ParseError::Format15),
        Revision::Format16 => format16::parse(bytes)
            .map(Container::Format16)
            .map_err(ParseError::Format16),
    }
}

#[cfg(test)]
mod tests {
    use super::{Container, DetectError, Detection, ParseError, Revision, detect, parse};
    use crate::format16::{self, BLOCK_HEADER_SIZE, FILE_HEADER_SIZE, Format16Error};

    fn fixture15() -> Vec<u8> {
        decode_base64(include_str!(
            "../../../tests/fixtures/cf/format15-clean-room.cf.b64"
        ))
    }

    fn fixture16() -> Vec<u8> {
        decode_base64(include_str!(
            "../../../tests/fixtures/cf/format16-clean-room.cf.b64"
        ))
    }

    #[test]
    fn detects_both_revisions_and_prioritizes_the_wide_base_signature() {
        assert_eq!(
            detect(&fixture15()).unwrap(),
            Detection {
                revision: Revision::Format15,
                base_offset: 0,
            }
        );
        assert_eq!(
            detect(&fixture16()).unwrap(),
            Detection {
                revision: Revision::Format16,
                base_offset: format16::BASE_OFFSET,
            }
        );
    }

    #[test]
    fn unified_parser_returns_expected_revision_names_and_counts() {
        let parsed15 = parse(&fixture15()).unwrap();
        let parsed16 = parse(&fixture16()).unwrap();

        assert_eq!(parsed15.revision(), Revision::Format15);
        assert_eq!(parsed16.revision(), Revision::Format16);
        assert_eq!(parsed15.element_count(), 5);
        assert_eq!(parsed16.element_count(), 5);
        assert_eq!(parsed16.base_offset(), format16::BASE_OFFSET);
        let Container::Format16(container) = parsed16 else {
            unreachable!();
        };
        assert_eq!(container.elements[2].name, "root");
        assert_eq!(container.elements[3].name, "version");
        assert_eq!(container.elements[4].name, "versions");
    }

    #[test]
    fn unknown_and_truncated_inputs_fail_closed_without_fallback() {
        assert_eq!(
            detect(&[1, 2, 3]).unwrap_err(),
            DetectError::InputTooShort {
                minimum: 4,
                actual: 3,
            }
        );
        assert_eq!(
            parse(&[0; 16]).unwrap_err(),
            ParseError::Detect(DetectError::UnknownSignature { found: 0 })
        );

        let mut truncated = fixture16();
        truncated.truncate(format16::BASE_OFFSET + FILE_HEADER_SIZE + BLOCK_HEADER_SIZE - 1);
        assert_eq!(
            parse(&truncated).unwrap_err(),
            ParseError::Format16(Format16Error::InputTooShort {
                minimum: FILE_HEADER_SIZE + BLOCK_HEADER_SIZE,
                actual: FILE_HEADER_SIZE + BLOCK_HEADER_SIZE - 1,
            })
        );
    }

    fn decode_base64(source: &str) -> Vec<u8> {
        let mut output = Vec::new();
        let mut buffer = 0_u32;
        let mut bits = 0_u8;
        for byte in source.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            if byte == b'=' {
                break;
            }
            let value = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => panic!("invalid fixture Base64 byte"),
            };
            buffer = (buffer << 6) | u32::from(value);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push(((buffer >> bits) & 0xff) as u8);
                buffer &= if bits == 0 { 0 } else { (1_u32 << bits) - 1 };
            }
        }
        output
    }
}
