//! Semantic Format16 preamble model and complete-archive writer.
//!
//! A standard Format16 artifact starts with a valid Format15 envelope whose
//! physical length is exactly `0x1359`. Generation is expressed as ordered,
//! named entries and deterministic tail padding; no opaque template blob is
//! embedded in production code.

use std::{error::Error, fmt, io::Write};

use ibcmd_v8::{
    format15, format16,
    writer::{
        Format15Document, Format15Element, Format16Document, Format16WriteReport,
        Format16WriterError, WriterError, format15_serialized_len, format16_serialized_len,
        write_format15_to_vec, write_format16_primary,
    },
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemanticPreambleEntry {
    pub name: String,
    /// `None` retains the absent-data distinction; `Some([])` is an explicit
    /// empty data block.
    pub data: Option<Vec<u8>>,
}

impl SemanticPreambleEntry {
    #[must_use]
    pub fn new(name: impl Into<String>, data: Option<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemanticPreamble {
    pub page_size: u32,
    pub storage_version: u32,
    pub reserved: u32,
    pub entries: Vec<SemanticPreambleEntry>,
}

impl SemanticPreamble {
    #[must_use]
    pub fn new(storage_version: u32, entries: Vec<SemanticPreambleEntry>) -> Self {
        Self {
            page_size: format15::DEFAULT_PAGE_SIZE,
            storage_version,
            reserved: 0,
            entries,
        }
    }

    #[must_use]
    pub fn from_container(container: &format15::Container) -> Self {
        Self {
            page_size: container.file_header.page_size,
            storage_version: container.file_header.storage_version,
            reserved: container.file_header.reserved,
            entries: container
                .elements
                .iter()
                .map(|element| {
                    SemanticPreambleEntry::new(element.name.clone(), element.data.clone())
                })
                .collect(),
        }
    }

    pub fn render(&self) -> Result<Vec<u8>, PreambleError> {
        let mut document = Format15Document::new(
            self.storage_version,
            self.entries
                .iter()
                .map(|entry| Format15Element::named(&entry.name, entry.data.clone()))
                .collect(),
        );
        document.page_size = self.page_size;
        document.reserved = self.reserved;
        let unpadded_size = format15_serialized_len(&document)?;
        let maximum = format16::BASE_OFFSET as u64;
        if unpadded_size > maximum {
            return Err(PreambleError::GeneratedTooLarge {
                actual: unpadded_size,
                maximum,
            });
        }
        document.tail_padding =
            u32::try_from(maximum - unpadded_size).map_err(|_| PreambleError::PaddingOverflow)?;
        let bytes = write_format15_to_vec(&document)?;
        if bytes.len() != format16::BASE_OFFSET {
            return Err(PreambleError::GeneratedSizeMismatch {
                expected: format16::BASE_OFFSET,
                actual: bytes.len(),
            });
        }
        format16::parse_preamble(&bytes).map_err(PreambleError::Invalid)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PreservedPreamble {
    raw: Vec<u8>,
    semantic: format15::Container,
}

impl PreservedPreamble {
    pub fn parse(raw: Vec<u8>) -> Result<Self, PreambleError> {
        let semantic = format16::parse_preamble(&raw).map_err(PreambleError::Invalid)?;
        Ok(Self { raw, semantic })
    }

    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    #[must_use]
    pub const fn semantic(&self) -> &format15::Container {
        &self.semantic
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PreambleMode {
    Preserve(PreservedPreamble),
    Generate(SemanticPreamble),
}

impl PreambleMode {
    pub fn preserve(raw: Vec<u8>) -> Result<Self, PreambleError> {
        PreservedPreamble::parse(raw).map(Self::Preserve)
    }

    #[must_use]
    pub const fn generate(model: SemanticPreamble) -> Self {
        Self::Generate(model)
    }

    pub fn render(&self) -> Result<Vec<u8>, PreambleError> {
        match self {
            Self::Preserve(preamble) => Ok(preamble.raw.clone()),
            Self::Generate(model) => model.render(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PreambleError {
    Invalid(format16::Format16Error),
    Writer(WriterError),
    GeneratedTooLarge { actual: u64, maximum: u64 },
    PaddingOverflow,
    GeneratedSizeMismatch { expected: usize, actual: usize },
}

impl fmt::Display for PreambleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => {
                write!(formatter, "invalid preserved Format16 preamble: {error}")
            }
            Self::Writer(error) => write!(formatter, "cannot generate Format16 preamble: {error}"),
            Self::GeneratedTooLarge { actual, maximum } => write!(
                formatter,
                "semantic Format16 preamble needs {actual} bytes, exceeding fixed base {maximum}"
            ),
            Self::PaddingOverflow => write!(
                formatter,
                "Format16 preamble padding does not fit in 32 bits"
            ),
            Self::GeneratedSizeMismatch { expected, actual } => write!(
                formatter,
                "generated Format16 preamble has {actual} bytes instead of {expected}"
            ),
        }
    }
}

impl Error for PreambleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Invalid(error) => Some(error),
            Self::Writer(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WriterError> for PreambleError {
    fn from(error: WriterError) -> Self {
        Self::Writer(error)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Format16ArchiveWriteReport {
    pub preamble_bytes: u64,
    pub primary: Format16WriteReport,
}

impl Format16ArchiveWriteReport {
    #[must_use]
    pub const fn bytes_written(self) -> u64 {
        self.preamble_bytes + self.primary.bytes_written
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Format16ArchiveWriterError {
    Preamble(PreambleError),
    Primary(Format16WriterError),
    ArtifactSizeOverflow,
    Output { kind: std::io::ErrorKind },
}

impl fmt::Display for Format16ArchiveWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preamble(error) => error.fmt(formatter),
            Self::Primary(error) => error.fmt(formatter),
            Self::ArtifactSizeOverflow => {
                write!(
                    formatter,
                    "complete Format16 artifact size overflows 64 bits"
                )
            }
            Self::Output { kind } => {
                write!(formatter, "Format16 archive preamble write failed: {kind}")
            }
        }
    }
}

impl Error for Format16ArchiveWriterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Preamble(error) => Some(error),
            Self::Primary(error) => Some(error),
            Self::ArtifactSizeOverflow => None,
            Self::Output { .. } => None,
        }
    }
}

impl From<PreambleError> for Format16ArchiveWriterError {
    fn from(error: PreambleError) -> Self {
        Self::Preamble(error)
    }
}

impl From<Format16WriterError> for Format16ArchiveWriterError {
    fn from(error: Format16WriterError) -> Self {
        Self::Primary(error)
    }
}

/// Writes a complete Format16 artifact after preflighting both the preamble
/// and all primary-container addresses. Payload sources remain unread until
/// both structural plans have succeeded.
pub fn write_format16_archive<W: Write>(
    target: &mut W,
    preamble: &PreambleMode,
    primary: &mut Format16Document<'_>,
) -> Result<Format16ArchiveWriteReport, Format16ArchiveWriterError> {
    let preamble_bytes = preamble.render()?;
    let primary_size = format16_serialized_len(primary)?;
    (preamble_bytes.len() as u64)
        .checked_add(primary_size)
        .ok_or(Format16ArchiveWriterError::ArtifactSizeOverflow)?;
    target
        .write_all(&preamble_bytes)
        .map_err(|error| Format16ArchiveWriterError::Output { kind: error.kind() })?;
    let primary_report = write_format16_primary(target, primary)?;
    Ok(Format16ArchiveWriteReport {
        preamble_bytes: preamble_bytes.len() as u64,
        primary: primary_report,
    })
}

pub fn write_format16_archive_to_vec(
    preamble: &PreambleMode,
    primary: &mut Format16Document<'_>,
) -> Result<Vec<u8>, Format16ArchiveWriterError> {
    let mut bytes = Vec::new();
    let report = write_format16_archive(&mut bytes, preamble, primary)?;
    debug_assert_eq!(report.bytes_written(), bytes.len() as u64);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use ibcmd_v8::{
        format16,
        writer::{Format16Document, Format16Element, Format16Payload},
    };

    use super::{
        PreambleMode, SemanticPreamble, SemanticPreambleEntry, write_format16_archive_to_vec,
    };

    fn semantic_preamble() -> SemanticPreamble {
        SemanticPreamble::new(
            5,
            vec![
                SemanticPreambleEntry::new("configuration", Some(b"header".to_vec())),
                SemanticPreambleEntry::new("root", Some(b"configuration".to_vec())),
                SemanticPreambleEntry::new("version", Some(b"80316".to_vec())),
                SemanticPreambleEntry::new("versions", Some(b"configuration:1".to_vec())),
            ],
        )
    }

    #[test]
    fn generated_preamble_is_exact_semantic_format15_envelope() {
        let model = semantic_preamble();
        let raw = model.render().unwrap();

        assert_eq!(raw.len(), format16::BASE_OFFSET);
        let parsed = format16::parse_preamble(&raw).unwrap();
        assert_eq!(
            parsed
                .elements
                .iter()
                .map(|element| element.name.as_str())
                .collect::<Vec<_>>(),
            ["configuration", "root", "version", "versions"]
        );
        assert_eq!(
            parsed.element("version").unwrap().data.as_deref(),
            Some(b"80316".as_slice())
        );
    }

    #[test]
    fn preserved_mode_validates_and_retains_exact_bytes() {
        let raw = semantic_preamble().render().unwrap();
        let mode = PreambleMode::preserve(raw.clone()).unwrap();
        assert_eq!(mode.render().unwrap(), raw);

        let error = PreambleMode::preserve(vec![0; format16::BASE_OFFSET - 1]).unwrap_err();
        assert!(error.to_string().contains("expected 4953 bytes"));
    }

    #[test]
    fn generated_complete_archive_roundtrips_through_format16_parser() {
        let payloads = [b"configuration-body".as_slice(), b"root-body".as_slice()];
        let mut primary = Format16Document::new(
            5,
            vec![
                Format16Element::named(
                    "configuration",
                    Some(Format16Payload::from_bytes(payloads[0])),
                ),
                Format16Element::named("missing", None),
                Format16Element::named("root", Some(Format16Payload::from_bytes(payloads[1]))),
            ],
        );
        let bytes = write_format16_archive_to_vec(
            &PreambleMode::generate(semantic_preamble()),
            &mut primary,
        )
        .unwrap();

        let parsed = format16::parse(&bytes).unwrap();
        assert_eq!(parsed.base_offset, format16::BASE_OFFSET);
        assert_eq!(parsed.elements.len(), 3);
        assert_eq!(
            parsed.element("configuration").unwrap().data.as_deref(),
            Some(payloads[0])
        );
        assert_eq!(parsed.element("missing").unwrap().data, None);
        assert_eq!(
            parsed.element("root").unwrap().data.as_deref(),
            Some(payloads[1])
        );
    }

    #[test]
    fn clean_room_archive_supports_exact_preserve_and_semantic_regeneration() {
        let fixture = decode_base64(include_str!(
            "../../../tests/fixtures/cf/format16-clean-room.cf.b64"
        ));
        let parsed = format16::parse(&fixture).unwrap();

        let mut preserved_primary = Format16Document::from_container(&parsed);
        let preserved = write_format16_archive_to_vec(
            &PreambleMode::preserve(parsed.raw_preamble.clone()).unwrap(),
            &mut preserved_primary,
        )
        .unwrap();
        assert_eq!(preserved, fixture);

        let semantic = SemanticPreamble::from_container(parsed.preamble.as_ref().unwrap());
        let mut regenerated_primary = Format16Document::from_container(&parsed);
        let regenerated = write_format16_archive_to_vec(
            &PreambleMode::generate(semantic),
            &mut regenerated_primary,
        )
        .unwrap();
        let reparsed = format16::parse(&regenerated).unwrap();
        assert_eq!(
            reparsed
                .elements
                .iter()
                .map(|element| (&element.name, &element.data))
                .collect::<Vec<_>>(),
            parsed
                .elements
                .iter()
                .map(|element| (&element.name, &element.data))
                .collect::<Vec<_>>()
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
