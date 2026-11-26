//! Helpers the readers use beside the vocabularies: the elapsed time of a
//! timing report, raw-deflate payloads, the file name of a picture payload and
//! whether a payload is SVG.

use std::time::Instant;

use anyhow::{Context, Result};
// The raw inflater is the module blob codec's.
use ibcmd_module_blob::inflate_raw;

pub fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

/// [`inflate_raw`], its error named `failed to inflate raw deflate blob` (the
/// `mssql_dump` readers).
pub fn inflate_raw_deflate(input: &[u8]) -> Result<Vec<u8>> {
    inflate_raw(input).context("failed to inflate raw deflate blob")
}

pub fn ext_picture_file_name(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "Picture.png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "Picture.gif"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "Picture.jpg"
    } else if bytes.starts_with(b"BM") {
        "Picture.bmp"
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        "Picture.ico"
    } else if bytes.starts_with(b"PK\x03\x04") {
        "Picture.zip"
    } else if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim_start_matches('\u{feff}').trim_start();
        if is_svg_text(text) {
            "Picture.svg"
        } else if trimmed.starts_with('<') {
            // A markup payload the platform does not recognise as an image is
            // written with no extension at all: ERP WE English
            // `CommonPictures/PictureBack` (an EDT metadata document stored as
            // the picture) is published `<xr:Abs>Picture</xr:Abs>`, and no
            // corpus publishes `Picture.xml`.
            "Picture"
        } else {
            "Picture.txt"
        }
    } else {
        "Picture.bin"
    }
}

pub fn is_svg_content(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    is_svg_text(text)
}

fn is_svg_text(text: &str) -> bool {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with("<svg") || text.starts_with("<?xml") && text.contains("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chain an error prints (`Error: …` and its `Caused by:` lines),
    /// and that the decoder's I/O error stays reachable under the context.
    #[test]
    fn a_stream_that_does_not_inflate_keeps_its_error_chain() {
        // A final block of the reserved type 3, which no stream holds.
        let corrupt = [0xff, 0xff];
        let raw = inflate_raw(&corrupt).unwrap_err();
        let named = inflate_raw_deflate(&corrupt).unwrap_err();
        let chain = |error: &anyhow::Error| -> Vec<String> {
            error.chain().map(ToString::to_string).collect()
        };
        assert_eq!(chain(&raw), ["corrupt deflate stream"]);
        assert_eq!(
            chain(&named),
            [
                "failed to inflate raw deflate blob",
                "corrupt deflate stream"
            ]
        );
        assert_eq!(format!("{named}"), "failed to inflate raw deflate blob");
        assert_eq!(
            format!("{named:#}"),
            "failed to inflate raw deflate blob: corrupt deflate stream"
        );
        assert!(raw.downcast_ref::<std::io::Error>().is_some());
        assert!(named.downcast_ref::<std::io::Error>().is_some());
    }

    #[test]
    fn a_raw_stream_inflates() {
        use flate2::{Compression, write::DeflateEncoder};
        use std::io::Write;
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"{1,\"\xd0\x81\"}").unwrap();
        let blob = encoder.finish().unwrap();
        assert_eq!(inflate_raw(&blob).unwrap(), b"{1,\"\xd0\x81\"}");
        assert_eq!(inflate_raw_deflate(&blob).unwrap(), b"{1,\"\xd0\x81\"}");
    }
}
