//! Byte-exact admission evidence for an initial 8.5 CommonForm module change.
//!
//! This helper does not enable an owner or publication capability. The caller
//! must also bind complete physical rows, the initial five-row cohort, root,
//! descriptor/version and unchanged registration preimages before publication.

use anyhow::{Context, Result, bail};
use flate2::{Decompress, FlushDecompress, Status};

use crate::module_blob::{
    FormBodyRevision, pack_form_body_blob_from_module_text, parse_form_body_plain,
};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;

const MAX_PLAIN: usize = 8 * 1024 * 1024;

fn inflate_complete(blob: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = Decompress::new(false);
    let mut plain = Vec::new();
    loop {
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let mut buffer = [0u8; 16 * 1024];
        let status = decoder.decompress(
            &blob[before_in as usize..],
            &mut buffer,
            FlushDecompress::Finish,
        )?;
        let produced = (decoder.total_out() - before_out) as usize;
        if plain.len().saturating_add(produced) > MAX_PLAIN {
            bail!("8.5 form decoded size exceeds {MAX_PLAIN}");
        }
        plain.extend_from_slice(&buffer[..produced]);
        if status == Status::StreamEnd {
            if decoder.total_in() != blob.len() as u64 {
                bail!("8.5 form has trailing compressed bytes");
            }
            return Ok(plain);
        }
        if decoder.total_in() == before_in && produced == 0 {
            bail!("8.5 form has an incomplete DEFLATE stream");
        }
    }
}

fn parse_measured_form(plain: &[u8]) -> Result<crate::module_blob::ParsedFormBodyBlob> {
    // Native tuples have a BOM, container revision 4, CR/LF and layout 59.
    // No outer padding, alternative frame revision or layout is inferred.
    if !plain.starts_with(b"\xef\xbb\xbf{4,\r\n{59,") || !plain.ends_with(b"}") {
        bail!("unmeasured 8.5 form frame or layout");
    }
    let text = std::str::from_utf8(plain).context("8.5 form is not UTF-8")?;
    let parsed = parse_form_body_plain(text)?;
    if parsed.revision != FormBodyRevision::V4 || !parsed.layout.starts_with("{59,") {
        bail!("unmeasured 8.5 form container or layout revision");
    }
    Ok(parsed)
}

/// Requires a genuine module edit and exactly equal bytes outside that field.
/// Immutable bytes are bounded before the existing parser/packer is invoked.
pub fn require_form_module_only_change(
    profile: MssqlNativePlatformProfile,
    active: &[u8],
    staged: &[u8],
) -> Result<()> {
    if profile != MssqlNativePlatformProfile::Platform8_5_1_1150 {
        bail!("8.5 form module admission is measured on 8.5.1.1150 only");
    }
    let old_plain = inflate_complete(active)?;
    let new_plain = inflate_complete(staged)?;
    let old = parse_measured_form(&old_plain)?;
    let new = parse_measured_form(&new_plain)?;
    if old.module_text == new.module_text {
        bail!("8.5 form module text is unchanged");
    }
    // The existing packer changes only its parsed module field. Comparing the
    // whole reconstructed native tuple also binds every opaque layout/trailer
    // byte, whitespace and quoting outside that field, without a second codec.
    let reconstructed = pack_form_body_blob_from_module_text(active, new.module_text.as_bytes())?;
    if inflate_complete(&reconstructed.blob)? != new_plain {
        bail!("8.5 form changed outside its canonical module text field");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::DeflateEncoder;
    use flate2::{Compress, Compression, FlushCompress};
    use std::io::Write;

    const PROFILE: MssqlNativePlatformProfile = MssqlNativePlatformProfile::Platform8_5_1_1150;
    const ACTIVE: &[u8] =
        include_bytes!("../../tests/fixtures/platform85-dynamic/form-active.native.deflate");
    const STAGED: &[u8] =
        include_bytes!("../../tests/fixtures/platform85-dynamic/form-staged.native.deflate");

    fn deflate(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn native_form_changes_only_its_module_and_stays_profile_specific() {
        require_form_module_only_change(PROFILE, ACTIVE, STAGED).unwrap();
        assert!(require_form_module_only_change(PROFILE, ACTIVE, ACTIVE).is_err());
        assert!(
            require_form_module_only_change(
                MssqlNativePlatformProfile::Platform8_3_27_2214,
                ACTIVE,
                STAGED
            )
            .is_err()
        );
        let old = inflate_complete(ACTIVE).unwrap();
        let new = inflate_complete(STAGED).unwrap();
        assert_eq!(old.len(), 8024);
        assert_eq!(
            String::from_utf8(old)
                .unwrap()
                .replace("P85_FORM_A", "P85_FORM_B")
                .as_bytes(),
            new
        );
    }

    #[test]
    fn rejects_changed_layout_trailer_and_unmeasured_frames() {
        let staged = String::from_utf8(inflate_complete(STAGED).unwrap()).unwrap();
        for invalid in [
            staged.replacen("{4,", "{3,", 1),
            staged.replacen("{59,", "{50,", 1),
            staged.replacen("Укажите примечание", "Укажите примечания", 1),
            staged.replacen("\r\n{59,", "\n{59,", 1),
            format!("{staged} "),
            format!("{staged},0"),
            staged.strip_prefix('\u{feff}').unwrap().to_owned(),
        ] {
            assert!(
                require_form_module_only_change(PROFILE, ACTIVE, &deflate(invalid.as_bytes()))
                    .is_err()
            );
        }
        let trailer_changed = staged.strip_suffix('}').unwrap().to_owned() + ",0}";
        assert!(
            require_form_module_only_change(PROFILE, ACTIVE, &deflate(trailer_changed.as_bytes()))
                .is_err()
        );
    }

    #[test]
    fn rejects_incomplete_trailing_invalid_and_oversized_streams() {
        for n in 0..STAGED.len() {
            assert!(
                require_form_module_only_change(PROFILE, ACTIVE, &STAGED[..n]).is_err(),
                "prefix {n}"
            );
        }
        let mut trailing = STAGED.to_vec();
        trailing.push(0);
        assert!(require_form_module_only_change(PROFILE, ACTIVE, &trailing).is_err());
        let plain = inflate_complete(STAGED).unwrap();
        let mut compressor = Compress::new(Compression::default(), false);
        let mut sync_flushed = vec![0; plain.len() + 1024];
        compressor
            .compress(&plain, &mut sync_flushed, FlushCompress::Sync)
            .unwrap();
        sync_flushed.truncate(compressor.total_out() as usize);
        assert_eq!(compressor.total_in(), plain.len() as u64);
        assert!(require_form_module_only_change(PROFILE, ACTIVE, &sync_flushed).is_err());
        assert!(inflate_complete(&deflate(&vec![0; MAX_PLAIN + 1])).is_err());
        let mut invalid_utf8 = plain;
        invalid_utf8[20] = 0xff;
        assert!(require_form_module_only_change(PROFILE, ACTIVE, &deflate(&invalid_utf8)).is_err());
    }
}
