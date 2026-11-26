//! The module blob: a raw-deflated V8 container of an `info` and a `text`
//! element, packed from module text (inheriting a base blob's element headers
//! and info when it has a `text` element) and unpacked to it, and the plain
//! module body some platforms store without the container.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};

use crate::raw::{deflate_raw, hex_sha256, inflate_raw};
use crate::v8_container::{
    V8Element, build_v8_container, make_v8_element_header, parse_v8_container, read_v8_element_data,
};

const DEFAULT_INFO: &[u8] = b"\xEF\xBB\xBF{3,1,0,\"\",0}";

#[derive(Debug, Clone)]
pub struct PackedModuleBlob {
    pub blob: Vec<u8>,
    pub info_bytes: usize,
    pub text_bytes: usize,
    pub inner_bytes: usize,
    pub output_sha256: String,
}

pub fn pack_module_blob_bytes(
    text: &[u8],
    base_blob: Option<&[u8]>,
    info: Option<&[u8]>,
) -> Result<PackedModuleBlob> {
    let base_elements = match base_blob {
        Some(blob) => {
            let elements = read_base_elements_from_blob(blob)?;
            elements.contains_key("text").then_some(elements)
        }
        None => None,
    };

    let info = match (info, &base_elements) {
        (Some(bytes), _) => bytes.to_vec(),
        (None, Some(elements)) => elements
            .get("info")
            .map(|element| element.data.clone())
            .unwrap_or_else(|| DEFAULT_INFO.to_vec()),
        (None, None) => DEFAULT_INFO.to_vec(),
    };

    let info_header = base_elements
        .as_ref()
        .and_then(|elements| elements.get("info"))
        .map(|element| element.header.clone())
        .unwrap_or_else(|| make_v8_element_header("info"));
    let text_header = base_elements
        .as_ref()
        .and_then(|elements| elements.get("text"))
        .map(|element| element.header.clone())
        .unwrap_or_else(|| make_v8_element_header("text"));

    let inner = build_module_inner(&[
        V8Element {
            name: "info".to_string(),
            header: info_header,
            data: info.clone(),
        },
        V8Element {
            name: "text".to_string(),
            header: text_header,
            data: text.to_vec(),
        },
    ])?;
    let blob = deflate_raw(&inner)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedModuleBlob {
        blob,
        info_bytes: info.len(),
        text_bytes: text.len(),
        inner_bytes: inner.len(),
        output_sha256,
    })
}

/// Packs module source without consulting or inheriting a base blob.
///
/// `info` must be independently supplied source data. When omitted, the
/// deterministic default info element and freshly generated element headers
/// are used.
pub fn pack_module_blob_bytes_base_free(
    text: &[u8],
    info: Option<&[u8]>,
) -> Result<PackedModuleBlob> {
    pack_module_blob_bytes(text, None, info)
}

pub fn pack_module_blob_container_bytes(container: &[u8]) -> Result<PackedModuleBlob> {
    let elements = parse_v8_container(container).context("failed to parse module V8 container")?;
    let info_bytes = elements
        .iter()
        .find(|element| element.name == "info")
        .map(|element| element.data.len())
        .unwrap_or_default();
    let text_bytes = elements
        .iter()
        .find(|element| element.name == "text")
        .map(|element| element.data.len())
        .unwrap_or_default();
    let blob = deflate_raw(container)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedModuleBlob {
        blob,
        info_bytes,
        text_bytes,
        inner_bytes: container.len(),
        output_sha256,
    })
}

pub fn unpack_module_blob_text(blob: &[u8]) -> Result<Vec<u8>> {
    let text = read_element_from_blob(blob, "text")
        .context("failed to read module blob text element")?
        .ok_or_else(|| anyhow!("module blob does not contain text element"))?;
    // Protected modules can retain a `text` element containing ciphertext.
    // It is not BSL source; exporting it would lose the binary container.
    std::str::from_utf8(&text).context("module text element is not UTF-8 source")?;
    Ok(text)
}

/// Recognizes a module body carrying no V8-container framing at all: the
/// row's own raw-deflate-inflated bytes *are* the final module text already,
/// BOM-prefixed exactly like the platform's own `Ext/Module.bsl`
/// (`docs/evidence/plain-text-module-body-lead-20260825.md`). Confirmed on
/// three real ERP УХ 3.2.12.6 `CommonModules`
/// (`БПМСФОУХ`/`ВариантыОтчетовПереопределяемый`/`ВзаиморасчетыВызовСервера`):
/// inflated bytes diffed byte-for-byte against the platform's own
/// `Ext/Module.bsl`, `diff` exit 0 on all three.
///
/// A first attempt at this fallback accepted any BOM-prefixed, valid-UTF-8
/// payload and was reverted: on `sslbase`/`ssl` it turned a form's Help-topic
/// XML wrapper (also BOM-prefixed, also valid UTF-8, by the same convention
/// every 1C text blob uses) into a spurious `Bots/<name>/Ext/Module.bsl`
/// through a `module_text_paths` collision the permissive check exposed but
/// did not cause -- see that doc for the full trace. BOM-and-valid-UTF8 alone
/// is not a safe enough discriminator: it is true of nearly every 1C text
/// blob this project reads, module or not.
///
/// The narrower, evidenced discriminator here: real BSL module source never
/// opens with `<`, `{` or `[` as its first non-whitespace byte -- those are
/// exactly the leading bytes of the wrapper shapes (`<?xml ...`, a JSON-ish
/// or brace-delimited record) this project's other plain-text blobs use, and
/// none of them are legal as the first token of a BSL module (there is no
/// top-level BSL construct that opens with a bare `<`, `{` or `[`; even a
/// `<` comparison operator needs a preceding operand). Every confirmed real
/// sample opens with a `//` line comment or a `#`-prefixed preprocessor
/// region instead. This is deliberately not a positive BSL-keyword
/// allowlist: legal BSL module bodies can open with many different
/// constructs (annotations, labels, bare statements), so requiring one
/// specific shape would fail closed on real modules; excluding the three
/// wrapper-shape bytes is the narrowest cut the current negative evidence
/// supports.
pub fn unpack_plain_text_module_body(blob: &[u8]) -> Option<Vec<u8>> {
    let inflated = inflate_raw(blob).ok()?;
    let after_bom = inflated.strip_prefix(b"\xEF\xBB\xBF")?;
    let text = std::str::from_utf8(after_bom).ok()?;
    let trimmed = text.trim_start_matches([' ', '\t', '\r', '\n']);
    if trimmed.starts_with(['<', '{', '[']) {
        return None;
    }
    Some(inflated)
}

pub fn unpack_module_container_text(container: &[u8]) -> Result<Vec<u8>> {
    let elements = parse_v8_container(container).context("failed to parse module V8 container")?;
    elements
        .into_iter()
        .find(|element| element.name == "text")
        .map(|element| element.data)
        .ok_or_else(|| anyhow!("module V8 container does not contain text element"))
}

pub fn module_blob_text_sha256(blob: &[u8]) -> Result<String> {
    let text = unpack_module_blob_text(blob)?;
    Ok(hex_sha256(&text))
}

fn read_base_elements_from_blob(blob: &[u8]) -> Result<BTreeMap<String, V8Element>> {
    let inner = inflate_raw(blob).context("failed to inflate base blob")?;
    let elements = parse_v8_container(&inner).context("failed to parse base blob")?;
    Ok(elements
        .into_iter()
        .map(|element| (element.name.clone(), element))
        .collect())
}

fn read_element_from_blob(blob: &[u8], target_name: &str) -> Result<Option<Vec<u8>>> {
    let inner = inflate_raw(blob).context("failed to inflate base blob")?;
    read_v8_element_data(&inner, target_name).context("failed to parse base blob")
}

fn build_module_inner(elements: &[V8Element; 2]) -> Result<Vec<u8>> {
    build_v8_container(elements)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_INFO, build_module_inner, deflate_raw, inflate_raw};
    use crate::v8_container::{V8Element, make_v8_element_header, parse_v8_container};

    fn decode_base64_for_test(input: &str) -> anyhow::Result<Vec<u8>> {
        fn value(byte: u8) -> anyhow::Result<u8> {
            match byte {
                b'A'..=b'Z' => Ok(byte - b'A'),
                b'a'..=b'z' => Ok(byte - b'a' + 26),
                b'0'..=b'9' => Ok(byte - b'0' + 52),
                b'+' => Ok(62),
                b'/' => Ok(63),
                _ => anyhow::bail!("invalid base64 byte {byte}"),
            }
        }

        let bytes: Vec<u8> = input
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
        for chunk in bytes.chunks(4) {
            if chunk.len() != 4 {
                anyhow::bail!("invalid base64 length");
            }
            let first = value(chunk[0])?;
            let second = value(chunk[1])?;
            let third = if chunk[2] == b'=' {
                0
            } else {
                value(chunk[2])?
            };
            let fourth = if chunk[3] == b'=' {
                0
            } else {
                value(chunk[3])?
            };
            output.push((first << 2) | (second >> 4));
            if chunk[2] != b'=' {
                output.push((second << 4) | (third >> 2));
            }
            if chunk[3] != b'=' {
                output.push((third << 6) | fourth);
            }
        }
        Ok(output)
    }

    #[test]
    fn packs_module_inner_with_plain_info_and_text() {
        let inner = build_module_inner(&[
            V8Element {
                name: "info".to_string(),
                header: make_v8_element_header("info"),
                data: DEFAULT_INFO.to_vec(),
            },
            V8Element {
                name: "text".to_string(),
                header: make_v8_element_header("text"),
                data: b"\xEF\xBB\xBFProcedure Test()\r\nEndProcedure".to_vec(),
            },
        ])
        .unwrap();

        assert_eq!(&inner[0..4], &0x7fff_ffff_u32.to_le_bytes());
        assert_eq!(&inner[8..12], &1_u32.to_le_bytes());

        let elements = parse_v8_container(&inner).unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].name, "info");
        assert_eq!(elements[0].data, DEFAULT_INFO);
        assert_eq!(elements[1].name, "text");
        assert!(elements[1].data.ends_with(b"EndProcedure"));
    }

    #[test]
    fn module_outer_blob_is_raw_deflate() {
        let inner = build_module_inner(&[
            V8Element {
                name: "info".to_string(),
                header: make_v8_element_header("info"),
                data: DEFAULT_INFO.to_vec(),
            },
            V8Element {
                name: "text".to_string(),
                header: make_v8_element_header("text"),
                data: b"text".to_vec(),
            },
        ])
        .unwrap();
        let blob = deflate_raw(&inner).unwrap();
        assert_eq!(inflate_raw(&blob).unwrap(), inner);
    }

    #[test]
    fn unpacks_module_blob_text_element() {
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let packed = super::pack_module_blob_bytes(text, None, None).unwrap();
        assert_eq!(super::unpack_module_blob_text(&packed.blob).unwrap(), text);
    }

    #[test]
    fn rejects_encrypted_module_text_element() {
        let packed = super::pack_module_blob_bytes(&[0xd2, 0xc6, 0x98, 0xff], None, None).unwrap();
        assert!(super::unpack_module_blob_text(&packed.blob).is_err());
    }

    /// Real ERP УХ 3.2.12.6 `CommonModules` module bodies carrying no V8
    /// container -- the row's raw-deflate-inflated bytes are the final,
    /// BOM-prefixed module text already
    /// (`docs/evidence/plain-text-module-body-lead-20260825.md`). Both
    /// fixtures are the exact `packed.bin` bytes `cf extract` produced for
    /// `CommonModules/БПМСФОУХ.0` (uuid `5b02973d-ada1-48eb-bf0a-
    /// f039f18b269d`, opens with a `//` comment) and
    /// `CommonModules/ВзаиморасчетыВызовСервера.0` (uuid `3004a91e-0ecc-
    /// 4c8b-9e3d-b901f848cff9`, opens with a `#Область` preprocessor
    /// region); both inflated bytes diffed byte-for-byte against the
    /// platform's own `Ext/Module.bsl` before being captured here.
    /// `unpack_module_blob_text` must reject both (no V8 container to find
    /// a `text` element in) while the new plain-text fallback must accept
    /// both, unchanged.
    #[test]
    fn unpacks_plain_text_module_body_real_common_modules() -> anyhow::Result<()> {
        let comment_led = decode_base64_for_test(include_str!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/plain-text-module-body/common-module-plain-text-bpmsfo.bin.b64"
        ))?;
        assert!(super::unpack_module_blob_text(&comment_led).is_err());
        let text = super::unpack_plain_text_module_body(&comment_led)
            .ok_or_else(|| anyhow::anyhow!("БПМСФОУХ fixture was not recognized as module text"))?;
        assert!(text.starts_with(b"\xEF\xBB\xBF//"));

        let region_led = decode_base64_for_test(include_str!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/plain-text-module-body/common-module-plain-text-vzr-region-directive.bin.b64"
        ))?;
        assert!(super::unpack_module_blob_text(&region_led).is_err());
        let text = super::unpack_plain_text_module_body(&region_led).ok_or_else(|| {
            anyhow::anyhow!("ВзаиморасчетыВызовСервера fixture was not recognized as module text")
        })?;
        assert!(text.starts_with(b"\xEF\xBB\xBF\r\n#"));
        Ok(())
    }

    /// Negative control for the regression the first (reverted) attempt at
    /// this fallback caused, captured from the exact colliding row named in
    /// `docs/evidence/plain-text-module-body-lead-20260825.md`: `sslbase`
    /// storage element `5a971dc0-28bf-4426-9426-9f456aea080a.1`, whose
    /// `module_text_paths` entry is a spurious `Bots/АрхивАнкет/Ext/
    /// Module.bsl` mapping (a form-classification false positive elsewhere
    /// in this crate, out of scope here) while the row itself is really
    /// `DataProcessors/ДоступныеАнкеты/Forms/АрхивАнкет/Ext/Help.xml`'s
    /// per-language content record. Confirmed (`cf extract` on
    /// `1cv8.cf` from `$D/kit/configs.sh`'s `sslbase` entry) that the row's
    /// raw-deflate-inflated bytes are BOM-prefixed, valid UTF-8, and *not*
    /// the `Help.xml` text itself but 1C's own typed-value wrapper around
    /// it: `{5,1,"ru",{#base64:...},0}` -- opening with `{`, one of the
    /// three wrapper-shape bytes `unpack_plain_text_module_body` excludes.
    /// A permissive "BOM + valid UTF-8" fallback (the reverted attempt)
    /// accepted this row as module text; this one must not.
    #[test]
    fn rejects_form_help_topic_braced_record_that_caused_the_reverted_regression()
    -> anyhow::Result<()> {
        let help_topic_record = decode_base64_for_test(include_str!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/plain-text-module-body/form-help-topic-braced-record.bin.b64"
        ))?;
        assert!(super::unpack_module_blob_text(&help_topic_record).is_err());
        assert!(super::unpack_plain_text_module_body(&help_topic_record).is_none());
        Ok(())
    }

    #[test]
    fn plain_text_module_body_rejects_xml_and_json_leads_accepts_bare_statement() {
        let wrap = |payload: &[u8]| deflate_raw(payload).unwrap();
        let xml = wrap(b"\xEF\xBB\xBF<?xml version=\"1.0\"?><Root/>");
        assert!(super::unpack_plain_text_module_body(&xml).is_none());

        let json = wrap(b"\xEF\xBB\xBF{\"a\":1}");
        assert!(super::unpack_plain_text_module_body(&json).is_none());

        let array = wrap(b"\xEF\xBB\xBF[1,2,3]");
        assert!(super::unpack_plain_text_module_body(&array).is_none());

        // No BOM at all: not the wire shape this fallback targets.
        let no_bom = wrap(b"Procedure Run()\r\nEndProcedure\r\n");
        assert!(super::unpack_plain_text_module_body(&no_bom).is_none());

        // Leading whitespace before a real statement is still accepted.
        let statement = wrap(b"\xEF\xBB\xBF  \r\nProcedure Run()\r\nEndProcedure\r\n");
        assert_eq!(
            super::unpack_plain_text_module_body(&statement).unwrap(),
            b"\xEF\xBB\xBF  \r\nProcedure Run()\r\nEndProcedure\r\n"
        );
    }

    #[test]
    fn closed_module_replacement_discards_closed_info_and_headers() -> anyhow::Result<()> {
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let mut header = super::make_v8_element_header("info");
        header[0] = 42;
        let closed = super::build_module_inner(&[
            super::V8Element {
                name: "info".into(),
                header,
                data: b"closed-module-info".to_vec(),
            },
            super::V8Element {
                name: "image".into(),
                header: super::make_v8_element_header("image"),
                data: vec![1, 2, 3],
            },
        ])?;
        let closed = super::deflate_raw(&closed)?;
        let replaced = super::pack_module_blob_bytes(text, Some(&closed), None)?;
        let fresh = super::pack_module_blob_bytes(text, None, None)?;
        assert_eq!(replaced.blob, fresh.blob);
        assert_eq!(super::unpack_module_blob_text(&replaced.blob)?, text);
        Ok(())
    }

    #[test]
    fn base_free_module_wrapper_matches_explicit_no_base_pack() -> anyhow::Result<()> {
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let info = b"{3,1,0,\"source-info\",0}";

        let wrapped = super::pack_module_blob_bytes_base_free(text, Some(info))?;
        let legacy = super::pack_module_blob_bytes(text, None, Some(info))?;

        assert_eq!(wrapped.blob, legacy.blob);
        assert_eq!(wrapped.info_bytes, info.len());
        assert_eq!(wrapped.text_bytes, text.len());
        assert_eq!(wrapped.inner_bytes, legacy.inner_bytes);
        assert_eq!(wrapped.output_sha256, legacy.output_sha256);
        assert_eq!(super::unpack_module_blob_text(&wrapped.blob)?, text);
        Ok(())
    }
}
