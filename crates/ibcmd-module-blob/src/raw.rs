//! Raw deflate (the stored form of every 1C body), the uppercase hex SHA-256
//! the reports print, and a raw-deflated body packed from its plain bytes or
//! digested by its plain bytes or its first base64 payload.

use std::io::{Read, Write};

use anyhow::{Context, Result, anyhow};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use ibcmd_1c_text::decode_base64_mime_lenient;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct PackedRawDeflatedBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

pub fn pack_raw_deflated_blob_from_bytes(bytes: &[u8]) -> Result<PackedRawDeflatedBlob> {
    let blob = deflate_raw(bytes)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: bytes.len(),
        output_sha256,
    })
}

pub fn raw_deflated_plain_sha256(blob: &[u8]) -> Result<String> {
    let plain = inflate_raw(blob).context("failed to inflate raw deflated blob")?;
    Ok(hex_sha256(&plain))
}

pub fn raw_deflated_first_base64_payload_sha256(blob: &[u8]) -> Result<String> {
    let plain =
        String::from_utf8(inflate_raw(blob).context("failed to inflate raw deflated base64 blob")?)
            .context("raw deflated base64 blob is not valid UTF-8")?;
    let prefix = "{#base64:";
    let payload_start = plain
        .find(prefix)
        .map(|index| index + prefix.len())
        .ok_or_else(|| anyhow!("raw deflated blob has no base64 payload"))?;
    let payload_end = plain[payload_start..]
        .find('}')
        .map(|relative| payload_start + relative)
        .ok_or_else(|| anyhow!("raw deflated blob base64 payload is not closed"))?;
    let payload = decode_base64_mime_lenient(&plain[payload_start..payload_end])
        .ok_or_else(|| anyhow!("raw deflated blob base64 payload is invalid"))?;
    Ok(hex_sha256(&payload))
}

/// A raw deflate stream inflated; a stream that does not inflate is the
/// decoder's I/O error alone, which callers name in their own context (the
/// root's `module_blob` readers, `ibcmd-1c-vocab`'s `inflate_raw_deflate`).
pub fn inflate_raw(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(input);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

pub fn deflate_raw(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input)?;
    encoder.finish().context("failed to finish deflate stream")
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    #[test]
    fn packs_raw_deflated_blob_from_bytes() -> anyhow::Result<()> {
        let bytes = b"<?xml version=\"1.0\"?><Definition><Item name=\"A\"/></Definition>";

        let packed = super::pack_raw_deflated_blob_from_bytes(bytes)?;

        assert_eq!(super::inflate_raw(&packed.blob)?, bytes);
        assert_eq!(packed.plain_bytes, bytes.len());

        Ok(())
    }

    #[test]
    fn hashes_first_base64_payload_from_raw_deflated_blob() -> anyhow::Result<()> {
        let left = super::deflate_raw(b"{1,{0},{{#base64:UEsDBA==}}}")?;
        let right = super::deflate_raw(b"{9,\"wrapper\",{#base64:UEsDBA==}}")?;

        assert_eq!(
            super::raw_deflated_first_base64_payload_sha256(&left)?,
            super::raw_deflated_first_base64_payload_sha256(&right)?
        );

        Ok(())
    }

    /// A stream that does not inflate is the decoder's I/O error alone: callers
    /// name it in their own context.
    #[test]
    fn a_stream_that_does_not_inflate_is_the_decoders_error_alone() {
        // A final block of the reserved type 3, which no stream holds.
        let error = super::inflate_raw(&[0xff, 0xff]).unwrap_err();
        let chain: Vec<String> = error.chain().map(ToString::to_string).collect();
        assert_eq!(chain, ["corrupt deflate stream"]);
        assert!(error.downcast_ref::<std::io::Error>().is_some());
    }
}
