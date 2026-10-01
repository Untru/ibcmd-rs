//! Complete source inspection without retaining a document or source buffer.
use super::*;
use std::io::{BufRead, Read, Seek, SeekFrom};

pub(super) fn inspect<R: BufRead + Seek>(mut input: R) -> Result<QName, XmlError> {
    let origin = input.stream_position().map_err(io_error)?;
    let (bom, length) = validate_bytes(&mut input)?;
    let parser_origin = origin
        .checked_add(u64::from(bom) * UTF8_BOM.len() as u64)
        .ok_or_else(|| {
            io_error(std::io::Error::other(
                "source byte position cannot be represented",
            ))
        })?;
    input
        .seek(SeekFrom::Start(parser_origin))
        .map_err(io_error)?;
    let result = inspect_events(&mut input, usize::from(bom) * UTF8_BOM.len(), length);
    match result {
        Ok(name) => Ok(name),
        Err((offset, cause)) => {
            let offset = offset.min(length);
            input.seek(SeekFrom::Start(origin)).map_err(io_error)?;
            let mut line = 1;
            let mut column = 1;
            let mut remaining = offset;
            let mut buffer = [0_u8; 8192];
            while remaining != 0 {
                let count = remaining.min(buffer.len());
                input.read_exact(&mut buffer[..count]).map_err(io_error)?;
                for byte in &buffer[..count] {
                    if *byte == b'\n' {
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                }
                remaining -= count;
            }
            Err(XmlError {
                offset,
                line,
                column,
                cause,
            })
        }
    }
}

fn io_error(error: std::io::Error) -> XmlError {
    XmlError {
        offset: 0,
        line: 1,
        column: 1,
        cause: XmlErrorCause::Parser(format!("source inspection I/O: {error}")),
    }
}

// Validate every source byte, including comments and ignored outside-root
// material. Keep the slice reader's UTF8 -> character -> BOM error priority.
fn validate_bytes<R: Read>(input: &mut R) -> Result<(bool, usize), XmlError> {
    let mut buffer = vec![0_u8; 65_536];
    let mut pending = Vec::new();
    let mut offset = 0usize;
    let mut line = 1usize;
    let mut column = 1usize;
    let mut character_error = None;
    let mut bom_error = None;
    let mut leading_bom = false;
    loop {
        let prefix = pending.len();
        buffer[..prefix].copy_from_slice(&pending);
        let read = input.read(&mut buffer[prefix..]).map_err(io_error)?;
        if read == 0 {
            if !pending.is_empty() {
                return Err(XmlError {
                    offset: 0,
                    line: 1,
                    column: 1,
                    cause: XmlErrorCause::InvalidUtf8,
                });
            }
            break;
        }
        let end = prefix + read;
        let valid_length = match std::str::from_utf8(&buffer[..end]) {
            Ok(_) => end,
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => {
                return Err(XmlError {
                    offset: 0,
                    line: 1,
                    column: 1,
                    cause: XmlErrorCause::InvalidUtf8,
                });
            }
        };
        pending.clear();
        pending.extend_from_slice(&buffer[valid_length..end]);
        for value in std::str::from_utf8(&buffer[..valid_length])
            .expect("validated UTF8 prefix")
            .chars()
        {
            if !valid_xml_char(value) && character_error.is_none() {
                character_error = Some(XmlError {
                    offset,
                    line,
                    column,
                    cause: XmlErrorCause::InvalidCharacter,
                });
            }
            if value == '\u{feff}' {
                if offset == 0 {
                    leading_bom = true;
                } else if bom_error.is_none() {
                    bom_error = Some(XmlError {
                        offset,
                        line,
                        column,
                        cause: XmlErrorCause::Parser(
                            "UTF-8 BOM is only allowed once at the beginning".into(),
                        ),
                    });
                }
            }
            offset = offset.checked_add(value.len_utf8()).ok_or_else(|| {
                io_error(std::io::Error::other(
                    "source byte position cannot be represented",
                ))
            })?;
            if value == '\n' {
                line += 1;
                column = 1;
            } else {
                column += value.len_utf8();
            }
        }
    }
    if let Some(error) = character_error.or(bom_error) {
        return Err(error);
    }
    Ok((leading_bom, offset))
}

type InspectionFailure = (usize, XmlErrorCause);

fn inspect_events<R: BufRead>(
    input: R,
    bom_bytes: usize,
    length: usize,
) -> Result<QName, InspectionFailure> {
    let mut reader = Reader::from_reader(input);
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    reader.config_mut().check_comments = true;
    let mut buffer = Vec::new();
    let mut stack = Vec::<QName>::new();
    let mut root = None;
    let mut declaration = false;
    let mut doctype = false;
    let mut seen_before = false;
    loop {
        let start = reader.buffer_position() as usize + bom_bytes;
        let event = reader.read_event_into(&mut buffer).map_err(|error| {
            (
                reader.error_position() as usize + bom_bytes,
                XmlErrorCause::Parser(error.to_string()),
            )
        })?;
        let end = reader.buffer_position() as usize + bom_bytes;
        match event {
            Event::Eof => break,
            Event::Decl(value) => {
                if !stack.is_empty() || root.is_some() || declaration || seen_before {
                    return Err((
                        start,
                        XmlErrorCause::UnsupportedOutsideRoot("XML declaration position".into()),
                    ));
                }
                let content =
                    std::str::from_utf8(value.as_ref()).expect("complete UTF8 validation");
                let raw = format!("<?{content}?>");
                validate_declaration(&[], 0, &raw).map_err(|error| (start, error.cause))?;
                declaration = true;
            }
            Event::Start(value) => {
                let bare = element(&[], 0, value.name().as_ref(), &value)
                    .map_err(|error| (start, error.cause))?;
                stack.push(bare.name);
            }
            Event::Empty(value) => {
                let bare = element(&[], 0, value.name().as_ref(), &value)
                    .map_err(|error| (start, error.cause))?;
                if stack.is_empty() && root.replace(bare.name).is_some() {
                    return Err((end, XmlErrorCause::MultipleRoots));
                }
            }
            Event::End(_) => {
                let name = stack
                    .pop()
                    .ok_or_else(|| (end, XmlErrorCause::Parser("unexpected closing tag".into())))?;
                if stack.is_empty() && root.replace(name).is_some() {
                    return Err((end, XmlErrorCause::MultipleRoots));
                }
            }
            Event::Text(value) => {
                let text = std::str::from_utf8(value.as_ref()).expect("complete UTF8 validation");
                let value = quick_xml::escape::unescape(text)
                    .map_err(|error| (end, XmlErrorCause::Parser(error.to_string())))?;
                if stack.is_empty() {
                    if !value.trim().is_empty() {
                        return Err((end, XmlErrorCause::TextOutsideRoot));
                    }
                    seen_before |= root.is_none();
                }
            }
            Event::GeneralRef(value) => {
                let text = std::str::from_utf8(value.as_ref()).expect("complete UTF8 validation");
                let value =
                    resolve_reference(&[], 0, text).map_err(|error| (start, error.cause))?;
                if stack.is_empty() {
                    if !value.trim().is_empty() {
                        return Err((end, XmlErrorCause::TextOutsideRoot));
                    }
                    seen_before |= root.is_none();
                }
            }
            Event::CData(_) if stack.is_empty() => {
                return Err((end, XmlErrorCause::TextOutsideRoot));
            }
            Event::CData(_) => {}
            Event::Comment(_) | Event::PI(_) => {
                seen_before |= stack.is_empty() && root.is_none();
            }
            Event::DocType(_) => {
                if !stack.is_empty() || root.is_some() || doctype {
                    return Err((start, XmlErrorCause::InvalidDocTypePosition));
                }
                doctype = true;
                seen_before = true;
            }
        }
        buffer.clear();
    }
    if let Some(name) = stack.last() {
        return Err((length, XmlErrorCause::UnclosedElement(name.raw().into())));
    }
    root.ok_or((length, XmlErrorCause::MissingRoot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    struct TinyReads<'a> {
        input: Cursor<&'a [u8]>,
        maximum: usize,
    }
    impl Read for TinyReads<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = output.len().min(self.maximum);
            self.input.read(&mut output[..count])
        }
    }
    impl Seek for TinyReads<'_> {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.input.seek(position)
        }
    }

    #[test]
    fn streaming_inspection_matches_document_checks_across_small_buffers() {
        let cases: &[&[u8]] = &[
            b"<r/>",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><r a=\"&#10;\">&#x20;<x/></r>",
            "\u{feff}<?xml version=\"1.0\"?><r>Кириллица Ελληνικά 🙂<!--text--><![CDATA[a<>]]></r>"
                .as_bytes(),
            b" \n<!--c--><?p a?><!DOCTYPE r><r/> \n<?after?><!---->",
            b"",
            b"<r>",
            b"<r/><s/>",
            b"<r/>bad",
            b"<![CDATA[ ]]><r/>",
            b"<r a=\"x\" a=\"y\"/>",
            b"<r a=\"&unknown;\"/>",
            b"<r>&unknown;</r>",
            b"<r>&#0;</r>",
            b"<r a=\"&#0;\"/>",
            b"<r>\0</r>",
            b"<r>\xff</r>",
            b"<r><!--x--x--></r>",
            b"<r><s></r>",
            b"<r/><!DOCTYPE r>",
            b"<r/><?xml version=\"1.0\"?>",
            b" <?xml version=\"1.0\"?><r/>",
            b"<?xml encoding=\"UTF-8\" version=\"1.0\"?><r/>",
            b"<?xml version=\"1.1\"?><r/>",
            b"<?xml version=\"1.0\" encoding=\"latin1\"?><r/>",
            "<r>\u{feff}</r>".as_bytes(),
            b"\xef\xbb\xbf\xef\xbb\xbf<r/>",
            b"<r>\0\xff</r>",
            b"<r>\0",
            b"<r>\xf0\x9f",
            b"<r>&#xD800;</r>",
        ];
        for &case in cases {
            let expected =
                XmlReader::from_slice(case).map(|document| document.root().name().clone());
            for capacity in [1, 2, 3, 7, 65_536] {
                let actual = inspect(BufReader::with_capacity(
                    capacity,
                    TinyReads {
                        input: Cursor::new(case),
                        maximum: capacity,
                    },
                ));
                assert_eq!(actual, expected, "capacity={capacity} bytes={case:?}");
            }
        }
    }

    #[test]
    fn utf8_and_forbidden_bom_are_checked_across_scan_chunk_boundaries() {
        let prefix = format!("<root>{}", " ".repeat(65_528));
        for suffix in [
            "🙂</root>",
            "🙂\0</root>",
            "\u{feff}</root>",
            "🙂<unclosed>",
        ] {
            let source = format!("{prefix}{suffix}");
            assert_eq!(
                inspect(Cursor::new(source.as_bytes())),
                XmlReader::from_slice(source.as_bytes())
                    .map(|document| document.root().name().clone())
            );
        }
        let source = [prefix.as_bytes(), &[0xf0, 0x9f], b"</root>"].concat();
        assert_eq!(
            inspect(Cursor::new(&source)),
            XmlReader::from_slice(&source).map(|document| document.root().name().clone())
        );
    }
}
