//! Attach the BLOB TEMPLATE-BODY SIDECARS — AddIn, BinaryData and HTMLDocument — onto the IR
//! `templates` during whole-config read, and re-emit them on write (§1.0/§1.3/§1.6). Covers
//! the CommonTemplate OBJECT and every `*.TemplateRef` CHILD. Mirror of
//! [`crate::template_read`] (TextDocument) / [`crate::mxl_read`] (MXL).
//!
//! # Why: edt/designer→cf needs these bodies (RE `ssl.cf`, all byte-exact via our own reader)
//! * **AddIn/BinaryData** — cf `<uuid>.0` = `BOM + {1,{#base64:<raw sidecar>}}`
//!   (`formats_cf::template_body::build_binary_template_body_image`; SSL 6/6 + 6/6).
//!   The canonical IR body = the RAW sidecar bytes VERBATIM (a ZIP/binary blob — no
//!   newline/BOM canonicalization). RE: the 10 witnessed sidecars are BYTE-IDENTICAL between
//!   the EDT and the Designer dump (sha1-compared 10/10) → one canon, no transcoding.
//!   A BinaryData template WITHOUT a sidecar (3/9 SSL — `МашиночитаемыеДоверенности`) is
//!   descriptor-only in cf → the body is OPTIONAL (attach iff the file exists).
//! * **HTMLDocument** — cf `<uuid>.0` = the HELP-body brace `{5,1,"ru",{#base64:BOM+CRLF(html)},0}`
//!   (`formats_cf::template_body::build_html_template_body_image`; SSL 12/12). The canonical
//!   IR body = the page BOM-stripped + EOL-normalized to `\n` (mirror of `help_read`).
//!
//! # Sidecar layout + encoding (RE: SSL/edt/src + SSL/designer_8.5.1/cf — 6 AddIn, 9 BinaryData,
//! # 12 HTMLDocument; every file below witnessed on disk in BOTH dumps)
//! The sidecar DIR is the dialect's usual template slot (sibling of
//! [`crate::template_read::child_sidecar_path`]):
//! * **EDT object** (`CommonTemplates/<N>/<N>.mdo`): siblings of the descriptor, `CommonTemplates/<N>/`.
//! * **EDT child** (`<Kind>s/<Host>/<Host>.mdo`): `<Kind>s/<Host>/Templates/<T>/`.
//! * **Designer object** (`CommonTemplates/<N>.xml`): `CommonTemplates/<N>/Ext/`.
//! * **Designer child** (`<Kind>s/<Host>.xml`): `<Kind>s/<Host>/Templates/<T>/Ext/`.
//!
//! Inside that dir the FILE NAMES and the ENCODING differ per dialect:
//!
//! | templateType | EDT                                | Designer                              |
//! |--------------|------------------------------------|---------------------------------------|
//! | AddIn        | `Template.addin` (raw)             | `Template.bin` (raw)                  |
//! | BinaryData   | `Template.bin` (raw)               | `Template.bin` (raw)                  |
//! | HTMLDocument | `<lang>.html` (BOM-less, **CRLF**) | `Template/<lang>.html` (**BOM**, **LF**) |
//! |              | + `Template.htmldoc` (g5 manifest) | + `Template.xml` (extrnprops manifest)|
//! |              | + `_files/<имя>` (raw, optional)   | + `Template/_files/<имя>` (raw, optional) |
//!
//! NB the AddIn extension is dialect-specific: EDT types it (`.addin`), Designer stores every
//! raw blob as `Template.bin`. The BYTES are identical — only the file name differs.
//!
//! NB the HTML EOL conventions are OPPOSITE (EDT=CRLF/no-BOM, Designer=LF/BOM) — exactly the
//! `help_read` situation, and the reverse of `.bsl` (CRLF in both). Hence the canon is
//! BOM-stripped + LF and each dialect re-encodes its own convention on write.
//!
//! ## The HTMLDocument page manifest
//! Both dialects declare the page set in a sidecar manifest, ONE `<lang>.html` per declared
//! page, in manifest order. SSL carries exactly `(ru)` (12/12); ERP is MULTI-LANG —
//! 44×`(en,ru)` + 71×`(ru)` — so the page set is MODELED (`Template::pages`), and the cf image
//! packs `{5,N,(lang,{#base64:…})×N,0}` (RE erp.cf Партнеры/Макет). Incoherent sidecars
//! (page without manifest, declared page without its file, malformed manifest, orphan
//! `_files/` without a manifest) → typed READ error (§1.0).
//! * EDT `Template.htmldoc` — the g5 `<htmldoc:HtmlDocument><pages lang="…"/>×N` shape
//!   ([`edt_htmldoc_manifest`] — the byte-canonical writer).
//! * Designer `Ext/Template.xml` — the `<Help …><Page>lang</Page>×N</Help>` extrnprops
//!   wrapper, BYTE-IDENTICAL to the object-help descriptor `Ext/Help.xml` shape →
//!   serialized/parsed by the SAME [`crate::help_read::serialize_page_descriptor`] /
//!   [`crate::help_read::parse_page_descriptor`], not a second copy of the shape.
//!
//! ## The HTMLDocument attached resources (`_files/` — RE `.fixtures/ERP`, 14 carriers/156 files)
//! Beside the pages either dialect MAY hold a `_files/` dir with the files the HTML embeds
//! (`<img src="_files/<имя>">`): EDT `<obj-dir>/_files/…`, Designer `Ext/Template/_files/…` —
//! bytes IDENTICAL between dialects (raw transport, no re-encoding), NOT declared by either
//! manifest (witnessed — mirror of the help `_files/` convention). Read into
//! `Template::resources` as `(rel-path-under-_files, raw bytes)` by the SAME sorted walk the
//! help resources use ([`crate::help_read::read_help_resources`] — its order == the cf tail
//! order, RE erp.cf 14/14); re-emitted verbatim on write. The cf image appends them as the
//! help-triplet tail and rewrites the in-HTML `src="_files/…"` prefix to the baked platform
//! uuid (`formats_cf::template_body` — cf-direction only; the XML dialects keep bare
//! `_files/`, witnessed in BOTH source trees).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::{MetadataObject, Template};

use crate::template_read::{is_template_ref_child, template_type_of};
use crate::ConvertError;

/// UTF-8 BOM (stripped from the HTML page for the canonical body; re-added for Designer).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// EDT sidecar file of an AddIn template body (Designer names it [`BIN_FILE`]).
const ADDIN_FILE: &str = "Template.addin";
/// Sidecar file of a BinaryData template body — and of an AddIn one in the Designer dialect.
const BIN_FILE: &str = "Template.bin";
/// EDT page-manifest of an HTMLDocument template.
const EDT_HTMLDOC_FILE: &str = "Template.htmldoc";
/// Designer page-manifest of an HTMLDocument template (the `Ext/Help.xml` wrapper shape).
const DESIGNER_HTMLDOC_FILE: &str = "Template.xml";
/// Designer subdir holding the HTML pages (`Ext/Template/<lang>.html`).
const DESIGNER_HTML_DIR: &str = "Template";

/// The canonical EDT `Template.htmldoc` manifest for the given page langs (ERP-witnessed:
/// 44×`(en,ru)` + 71×`(ru)` manifests are BYTE-IDENTICAL to this shape — CRLF, two-space
/// indent, one self-closed `<pages lang="…"/>` per page in manifest order, trailing CRLF).
fn edt_htmldoc_manifest(langs: &[String]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
         <htmldoc:HtmlDocument xmlns:htmldoc=\"http://g5.1c.ru/v8/dt/html-document\">\r\n",
    );
    for lang in langs {
        out.push_str("  <pages lang=\"");
        out.push_str(lang);
        out.push_str("\"/>\r\n");
    }
    out.push_str("</htmldoc:HtmlDocument>\r\n");
    out
}

/// Which blob family (if any) does this `templateType` belong to?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlobKind {
    /// Raw-binary transport, EDT `Template.addin` / Designer `Template.bin` — verbatim bytes.
    AddIn,
    /// Raw-binary transport, `Template.bin` in BOTH dialects — verbatim bytes.
    BinaryData,
    /// HTML page + page manifest.
    Html,
}

fn blob_kind(ttype: &str) -> Option<BlobKind> {
    match ttype {
        "AddIn" => Some(BlobKind::AddIn),
        "BinaryData" => Some(BlobKind::BinaryData),
        "HTMLDocument" => Some(BlobKind::Html),
        _ => None,
    }
}

impl BlobKind {
    /// Raw-binary sidecar file name in `format`, or `None` for [`BlobKind::Html`] (a pair).
    fn binary_file(self, format: Format) -> Option<&'static str> {
        match (self, format) {
            (BlobKind::AddIn, Format::Designer) | (BlobKind::BinaryData, _) => Some(BIN_FILE),
            (BlobKind::AddIn, _) => Some(ADDIN_FILE),
            (BlobKind::Html, _) => None,
        }
    }
}

/// Manifest path of an HTMLDocument body inside its sidecar `dir`, per dialect.
fn html_manifest_path(format: Format, dir: &Path) -> PathBuf {
    match format {
        Format::Designer => dir.join(DESIGNER_HTMLDOC_FILE),
        // EDT (cf never reaches a file path — `sidecar_dir` declines it first).
        _ => dir.join(EDT_HTMLDOC_FILE),
    }
}

/// Page file path of ONE HTML page (`<lang>.html`), per dialect layout.
fn html_page_path(format: Format, dir: &Path, lang: &str) -> PathBuf {
    let file = format!("{lang}.html");
    match format {
        Format::Designer => dir.join(DESIGNER_HTML_DIR).join(file),
        _ => dir.join(file),
    }
}

/// Каталог файлов-ресурсов `_files/` HTMLDocument-макета: СОСЕД страниц (module docs —
/// EDT `<sidecar-dir>/_files/`, Designer `<sidecar-dir>/Template/_files/`).
fn html_files_dir(format: Format, dir: &Path) -> PathBuf {
    match format {
        Format::Designer => dir.join(DESIGNER_HTML_DIR).join(FILES_DIR),
        _ => dir.join(FILES_DIR),
    }
}

/// Имя каталога файлов-ресурсов рядом со страницами (== help-конвенции `_files`).
const FILES_DIR: &str = "_files";

/// Does the dialect's page location hold ANY `*.html` file? (Orphan-page detector for the
/// manifest-less case — a page without a manifest is an incoherent sidecar, §1.0.)
fn html_any_page_present(format: Format, dir: &Path) -> bool {
    let page_dir = match format {
        Format::Designer => dir.join(DESIGNER_HTML_DIR),
        _ => dir.to_path_buf(),
    };
    std::fs::read_dir(&page_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.path().extension().map(|x| x == "html").unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Sidecar DIR of a blob template body: the OBJECT's own dir (`child_name == None`,
/// CommonTemplate) or a `*.TemplateRef` CHILD's dir under its host — per dialect layout
/// (module docs). `None` for cf (a container, not a file tree).
fn sidecar_dir(format: Format, descriptor: &Path, child_name: Option<&str>) -> Option<PathBuf> {
    const TEMPLATES_DIR: &str = "Templates";
    let dir = descriptor.parent()?;
    match (format, child_name) {
        // EDT object: siblings of `<N>.mdo`.
        (Format::Edt, None) => Some(dir.to_path_buf()),
        // EDT child: `<host-dir>/Templates/<T>/`.
        (Format::Edt, Some(t)) => Some(dir.join(TEMPLATES_DIR).join(t)),
        // Designer object: `<dir>/<N>/Ext/` (the descriptor is the FILE `<N>.xml`).
        (Format::Designer, None) => Some(dir.join(descriptor.file_stem()?).join("Ext")),
        // Designer child: `<dir>/<host-stem>/Templates/<T>/Ext/`.
        (Format::Designer, Some(t)) => Some(
            dir.join(descriptor.file_stem()?)
                .join(TEMPLATES_DIR)
                .join(t)
                .join("Ext"),
        ),
        (Format::Cf, _) => None,
    }
}

/// Parse the EDT `Template.htmldoc` manifest into its page langs, IN MANIFEST ORDER
/// (whitespace-tolerant: each `<pages` element must be self-closed and carry `lang="…"`).
/// `None` — malformed manifest (missing lang / no pages).
fn edt_htmldoc_langs(manifest: &str) -> Option<Vec<String>> {
    let mut langs = Vec::new();
    for (i, _) in manifest.match_indices("<pages") {
        let rest = &manifest[i..];
        let end = rest.find("/>")?;
        let tag = &rest[..end];
        let li = tag.find("lang=\"")? + "lang=\"".len();
        let lv = &tag[li..];
        let lq = lv.find('\"')?;
        langs.push(lv[..lq].to_string());
    }
    if langs.is_empty() {
        return None;
    }
    Some(langs)
}

/// Read the HTMLDocument page MANIFEST into its ordered lang list (§1.0: empty/malformed
/// manifest → typed refusal). Multi-page sets are MODELED (ERP-witnessed 44×`(en,ru)`).
fn html_manifest_langs(
    format: Format,
    manifest: &Path,
    target: &MetadataObject,
) -> Result<Vec<String>, ConvertError> {
    let langs = match format {
        // Designer shares the `<Help><Page>lang</Page></Help>` wrapper with the object help.
        Format::Designer => crate::help_read::parse_page_descriptor(
            target.kind.as_str(),
            manifest,
            &target.name,
        )?,
        _ => {
            let text = std::fs::read_to_string(manifest).map_err(|e| ConvertError::Io {
                path: manifest.display().to_string(),
                reason: e.to_string(),
            })?;
            edt_htmldoc_langs(&text).ok_or_else(|| ConvertError::Read {
                kind: target.kind.as_str().to_string(),
                object: target.name.clone(),
                reason: format!(
                    "HTMLDocument template manifest {} is malformed (no parsable \
                     `<pages lang=…/>` entries) — §1.0",
                    manifest.display()
                ),
            })?
        }
    };
    if langs.is_empty() {
        return Err(ConvertError::Read {
            kind: target.kind.as_str().to_string(),
            object: target.name.clone(),
            reason: format!(
                "HTMLDocument template manifest {} declares no pages — §1.0",
                manifest.display()
            ),
        });
    }
    Ok(langs)
}

/// Attach a blob template body to `target.templates` from `dir/<files>`. OPTIONAL for the raw
/// binaries (bodyless stub → descriptor-only); for Html the PAIR must be coherent (manifest
/// present ⇔ page present) and the manifest must declare the witnessed single `ru` page (§1.0).
fn attach_one(
    kind: BlobKind,
    format: Format,
    dir: &Path,
    target: &mut MetadataObject,
) -> Result<(), ConvertError> {
    let body: Option<Vec<u8>> = match kind.binary_file(format) {
        Some(file) => {
            let path = dir.join(file);
            if !path.is_file() {
                None // bodyless stub — descriptor-only round-trip (witnessed 3/9 BinaryData).
            } else {
                // Raw blob: VERBATIM bytes (identical in both dialects — no transcoding).
                Some(std::fs::read(&path).map_err(|e| ConvertError::Io {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                })?)
            }
        }
        None => {
            let manifest = html_manifest_path(format, dir);
            if !manifest.is_file() {
                if html_any_page_present(format, dir) || html_files_dir(format, dir).is_dir() {
                    return Err(ConvertError::Read {
                        kind: target.kind.as_str().to_string(),
                        object: target.name.clone(),
                        reason: format!(
                            "HTMLDocument template sidecar is incoherent (a page file or a \
                             `_files/` resource dir is present but the manifest {} is \
                             missing) — §1.0",
                            manifest.display()
                        ),
                    });
                }
                return Ok(()); // bodyless stub (coverage s4 HTML property-stub).
            }
            let langs = html_manifest_langs(format, &manifest, target)?;
            let mut pages = Vec::with_capacity(langs.len());
            for lang in &langs {
                let page = html_page_path(format, dir, lang);
                if !page.is_file() {
                    return Err(ConvertError::Read {
                        kind: target.kind.as_str().to_string(),
                        object: target.name.clone(),
                        reason: format!(
                            "HTMLDocument template sidecar is incoherent (manifest declares \
                             page `{lang}` but {} is missing) — §1.0",
                            page.display()
                        ),
                    });
                }
                let bytes = std::fs::read(&page).map_err(|e| ConvertError::Io {
                    path: page.display().to_string(),
                    reason: e.to_string(),
                })?;
                // Canonical body: BOM-stripped + EOL→LF (mirror of `help_read`; EDT is
                // CRLF/BOM-less on disk, Designer is LF/BOM'd — opposite conventions).
                let stripped = bytes.strip_prefix(BOM).unwrap_or(&bytes);
                pages.push((
                    lang.clone(),
                    crate::template_read::normalize_newlines(stripped, false),
                ));
            }
            // Attached `_files/` resources beside the pages (OPTIONAL; raw verbatim bytes,
            // sorted walk == cf tail order — module docs). Reuses the help `_files/` reader
            // (same layout convention, same canon; §1.0 typed refusals inside).
            let files_dir = html_files_dir(format, dir);
            let resources: Vec<(String, Vec<u8>)> = if files_dir.is_dir() {
                crate::help_read::read_help_resources(
                    target.kind.as_str(),
                    &target.name,
                    &files_dir,
                )?
                .into_iter()
                .map(|r| (r.rel_path, r.bytes))
                .collect()
            } else {
                Vec::new()
            };
            // §1.0: only this pass populates a blob template's `templates`.
            if !target.templates.is_empty() {
                return Err(ConvertError::Read {
                    kind: target.kind.as_str().to_string(),
                    object: target.name.clone(),
                    reason: "object already carries templates before the blob sidecar attach \
                             (unexpected — the descriptor projection must not populate them)"
                        .into(),
                });
            }
            target.templates.push(Template {
                name: target.name.clone(),
                properties: Vec::new(),
                pages,
                resources,
                body: None,
            });
            return Ok(());
        }
    };
    let Some(body) = body else { return Ok(()) };
    // §1.0: only this pass populates a blob template's `templates`.
    if !target.templates.is_empty() {
        return Err(ConvertError::Read {
            kind: target.kind.as_str().to_string(),
            object: target.name.clone(),
            reason: "object already carries templates before the blob sidecar attach \
                     (unexpected — the descriptor projection must not populate them)"
                .into(),
        });
    }
    target.templates.push(Template {
        pages: Vec::new(),
        resources: Vec::new(),
        name: target.name.clone(),
        properties: Vec::new(),
        body: Some(body),
    });
    Ok(())
}

/// Load the AddIn/BinaryData/HTMLDocument template bodies (object + `*.TemplateRef` children)
/// from their sidecars — see module docs. Both XML dialects; cf is a read no-op (a container).
pub fn attach_blob_template_bodies(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(());
    }
    // (a) `*.TemplateRef` children (sidecars under the HOST dir).
    for child in obj.children.iter_mut() {
        if !is_template_ref_child(child) {
            continue;
        }
        let Some(bk) = blob_kind(template_type_of(child)) else {
            continue;
        };
        let Some(dir) = sidecar_dir(format, descriptor_path, Some(&child.name)) else {
            continue;
        };
        attach_one(bk, format, &dir, child)?;
    }
    // (b) CommonTemplate object-level body (its own sidecar dir).
    if kind != "CommonTemplate" {
        return Ok(());
    }
    let Some(bk) = blob_kind(template_type_of(obj)) else {
        return Ok(());
    };
    let Some(dir) = sidecar_dir(format, descriptor_path, None) else {
        return Ok(());
    };
    attach_one(bk, format, &dir, obj)
}

/// Write-side mirror of [`attach_blob_template_bodies`]: re-emit each attached blob body in the
/// TARGET dialect's layout + encoding (module docs). Bodyless objects/children and cf are no-ops.
pub fn write_blob_template_bodies(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(());
    }
    // (a) children.
    for child in &obj.children {
        if !is_template_ref_child(child) {
            continue;
        }
        let Some(bk) = blob_kind(template_type_of(child)) else {
            continue;
        };
        let Some(dir) = sidecar_dir(format, descriptor_out, Some(&child.name)) else {
            continue;
        };
        if bk == BlobKind::Html {
            if let Some(t) = child.templates.first() {
                if !t.pages.is_empty() {
                    write_html(format, &dir, &t.pages, &t.resources)?;
                }
            }
            continue;
        }
        let Some(body) = child.templates.first().and_then(|t| t.body.as_deref()) else {
            continue;
        };
        write_one(bk, format, &dir, body)?;
    }
    // (b) object-level.
    if kind != "CommonTemplate" {
        return Ok(());
    }
    let Some(bk) = blob_kind(template_type_of(obj)) else {
        return Ok(());
    };
    let Some(dir) = sidecar_dir(format, descriptor_out, None) else {
        return Ok(());
    };
    if bk == BlobKind::Html {
        if let Some(t) = obj.templates.first() {
            if !t.pages.is_empty() {
                write_html(format, &dir, &t.pages, &t.resources)?;
            }
        }
        return Ok(());
    }
    let Some(body) = obj.templates.first().and_then(|t| t.body.as_deref()) else {
        return Ok(());
    };
    write_one(bk, format, &dir, body)
}

/// Emit ONE raw-binary blob body into `dir` in `format`'s layout — VERBATIM bytes.
fn write_one(kind: BlobKind, format: Format, dir: &Path, body: &[u8]) -> Result<(), ConvertError> {
    let Some(file) = kind.binary_file(format) else {
        unreachable!("BlobKind::Html is routed to write_html by the caller");
    };
    crate::form_write::write_file(&dir.join(file), body)
}

/// Emit an HTMLDocument body: each page re-encoded to the dialect's convention (EDT: BOM-less
/// CRLF; Designer: BOM'd LF) beside the dialect's canonical manifest listing the page langs in
/// IR (manifest) order, plus the `_files/<rel>` resources RAW VERBATIM beside the pages
/// (module docs; both source trees hold identical bytes — no re-encoding).
fn write_html(
    format: Format,
    dir: &Path,
    pages: &[(String, Vec<u8>)],
    resources: &[(String, Vec<u8>)],
) -> Result<(), ConvertError> {
    let langs: Vec<String> = pages.iter().map(|(l, _)| l.clone()).collect();
    for (lang, body) in pages {
        let page = html_page_path(format, dir, lang);
        match format {
            Format::Designer => {
                // Canon is LF → Designer wants BOM + LF (verbatim after the BOM).
                let mut bytes = BOM.to_vec();
                bytes.extend_from_slice(body);
                crate::form_write::write_file(&page, &bytes)?;
            }
            // EDT: BOM-less, CRLF (the canon carries no `\r\n` — expansion cannot double a CR).
            _ => {
                let crlf = crate::template_read::normalize_newlines(body, true);
                crate::form_write::write_file(&page, &crlf)?;
            }
        }
    }
    let manifest = html_manifest_path(format, dir);
    match format {
        Format::Designer => {
            let lang_refs: Vec<&str> = langs.iter().map(String::as_str).collect();
            crate::form_write::write_file(
                &manifest,
                &crate::help_read::serialize_page_descriptor(&lang_refs),
            )?;
        }
        _ => {
            crate::form_write::write_file(&manifest, edt_htmldoc_manifest(&langs).as_bytes())?;
        }
    }
    // `_files/<rel>` — сырые байты дословно (rel — `/`-разделённый канон; `join` на Windows
    // принимает `/` как разделитель — та же конвенция, что у help-ресурсов).
    let files_dir = html_files_dir(format, dir);
    for (rel, bytes) in resources {
        crate::form_write::write_file(&files_dir.join(rel), bytes)?;
    }
    Ok(())
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::{PropertyValue, Token};
    use morph1c_core::ir::{ObjectKind, Uuid};
    use morph1c_core::spec::metadata::common_template::F_TEMPLATE_TYPE;

    fn tmpl(kind: &str, ttype: &str, name: &str) -> MetadataObject {
        let mut o = MetadataObject::new(ObjectKind::new(kind), name, Uuid([1; 16]));
        o.properties
            .push((F_TEMPLATE_TYPE, PropertyValue::Enum(Token::new(ttype))));
        o
    }

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-blob-tmpl-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn edt_htmldoc_manifest_parses_and_serializes_round_trip() {
        // Single-`ru` (SSL 12/12) and multi-`(en,ru)` (ERP 44) — both byte-canonical shapes.
        for langs in [vec!["ru".to_string()], vec!["en".to_string(), "ru".to_string()]] {
            let text = edt_htmldoc_manifest(&langs);
            assert_eq!(edt_htmldoc_langs(&text), Some(langs), "{text}");
        }
        // Malformed shapes are None (no parsable pages / missing lang).
        assert_eq!(edt_htmldoc_langs("<HtmlDocument/>"), None);
        assert_eq!(edt_htmldoc_langs("<pages/>"), None);
    }

    /// RE: the Designer HTMLDocument manifest is BYTE-IDENTICAL to the 232-byte `Ext/Help.xml`
    /// single-`ru` descriptor (12/12 SSL manifests) — one serializer, one shape.
    #[test]
    fn designer_manifest_is_the_232_byte_help_wrapper() {
        let bytes = crate::help_read::serialize_page_descriptor(&["ru"]);
        assert_eq!(bytes.len(), 232, "witnessed SSL manifest size");
        assert!(bytes.starts_with(BOM));
        assert!(bytes.ends_with(b"\t<Page>ru</Page>\r\n</Help>"));
    }

    /// The AddIn sidecar is `Template.addin` in EDT but `Template.bin` in Designer; BinaryData
    /// is `Template.bin` in both. HTML has no single binary file (it is a page+manifest pair).
    #[test]
    fn binary_sidecar_name_is_dialect_specific() {
        assert_eq!(BlobKind::AddIn.binary_file(Format::Edt), Some(ADDIN_FILE));
        assert_eq!(
            BlobKind::AddIn.binary_file(Format::Designer),
            Some(BIN_FILE)
        );
        assert_eq!(
            BlobKind::BinaryData.binary_file(Format::Edt),
            Some(BIN_FILE)
        );
        assert_eq!(
            BlobKind::BinaryData.binary_file(Format::Designer),
            Some(BIN_FILE)
        );
        assert_eq!(BlobKind::Html.binary_file(Format::Edt), None);
    }

    /// The witnessed sidecar DIRs, both dialects, object + child.
    #[test]
    fn sidecar_dirs_per_format() {
        // EDT object: siblings of the `.mdo`.
        let edt_obj = Path::new("src/CommonTemplates/К/К.mdo");
        assert_eq!(
            sidecar_dir(Format::Edt, edt_obj, None).unwrap(),
            Path::new("src/CommonTemplates/К")
        );
        // EDT child: `<host-dir>/Templates/<T>/`.
        let edt_host = Path::new("src/Catalogs/Спр/Спр.mdo");
        assert_eq!(
            sidecar_dir(Format::Edt, edt_host, Some("М")).unwrap(),
            Path::new("src/Catalogs/Спр/Templates/М")
        );
        // Designer object: `<N>/Ext/`.
        let des_obj = Path::new("cf/CommonTemplates/К.xml");
        assert_eq!(
            sidecar_dir(Format::Designer, des_obj, None).unwrap(),
            Path::new("cf/CommonTemplates/К/Ext")
        );
        // Designer child: `<host>/Templates/<T>/Ext/`.
        let des_host = Path::new("cf/Catalogs/Спр.xml");
        assert_eq!(
            sidecar_dir(Format::Designer, des_host, Some("М")).unwrap(),
            Path::new("cf/Catalogs/Спр/Templates/М/Ext")
        );
        assert!(sidecar_dir(Format::Cf, edt_obj, None).is_none());
    }

    #[test]
    fn attaches_edt_addin_verbatim_and_binary_optional() {
        let base = scratch("addin");
        let obj_dir = base.join("CommonTemplates").join("Компонента");
        std::fs::create_dir_all(&obj_dir).unwrap();
        crate::fsio::write(obj_dir.join(ADDIN_FILE), b"PK\x03\x04raw-zip-bytes\r\n\x00").unwrap();
        let descriptor = obj_dir.join("Компонента.mdo");

        let mut obj = tmpl("CommonTemplate", "AddIn", "Компонента");
        attach_blob_template_bodies(Format::Edt, "CommonTemplate", &descriptor, &mut obj).unwrap();
        assert_eq!(
            obj.templates[0].body.as_deref(),
            Some(b"PK\x03\x04raw-zip-bytes\r\n\x00".as_slice()),
            "raw sidecar bytes verbatim (no canonicalization)"
        );

        // Bodyless BinaryData stub attaches nothing (descriptor-only, witnessed 3/9 SSL).
        let mut stub = tmpl("CommonTemplate", "BinaryData", "Пустой");
        attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &base
                .join("CommonTemplates")
                .join("Пустой")
                .join("Пустой.mdo"),
            &mut stub,
        )
        .unwrap();
        assert!(stub.templates.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    /// The SSL witness in miniature: an AddIn body written to Designer lands at
    /// `<N>/Ext/Template.bin` carrying the SAME bytes the EDT `Template.addin` does (the 10/10
    /// sha1-identical sidecar pairs), and reads back to the identical canon.
    #[test]
    fn addin_round_trips_edt_to_designer_and_back() {
        let base = scratch("addin-x");
        let raw: &[u8] = b"PK\x03\x04\x00\xffbinary\r\n\x00blob";

        // EDT source → canon.
        let edt_dir = base.join("edt").join("CommonTemplates").join("К");
        std::fs::create_dir_all(&edt_dir).unwrap();
        crate::fsio::write(edt_dir.join(ADDIN_FILE), raw).unwrap();
        let mut obj = tmpl("CommonTemplate", "AddIn", "К");
        attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir.join("К.mdo"),
            &mut obj,
        )
        .unwrap();
        assert_eq!(obj.templates[0].body.as_deref(), Some(raw));

        // canon → Designer: `CommonTemplates/К/Ext/Template.bin`, bytes VERBATIM.
        let des_root = base.join("des").join("CommonTemplates");
        std::fs::create_dir_all(&des_root).unwrap();
        let des_desc = des_root.join("К.xml");
        write_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &obj).unwrap();
        let written = des_root.join("К").join("Ext").join(BIN_FILE);
        assert_eq!(
            std::fs::read(&written).unwrap(),
            raw,
            "verbatim across dialects"
        );

        // Designer → canon again (same body).
        let mut back = tmpl("CommonTemplate", "AddIn", "К");
        attach_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &mut back)
            .unwrap();
        assert_eq!(back.templates[0].body.as_deref(), Some(raw));

        let _ = std::fs::remove_dir_all(&base);
    }

    /// HTMLDocument: EDT is BOM-less CRLF, Designer is BOM'd LF, canon is BOM-less LF — and the
    /// manifests differ (`Template.htmldoc` vs `Ext/Template.xml`). Both directions byte-exact.
    #[test]
    fn html_round_trips_both_dialects_with_opposite_eol() {
        let base = scratch("html-x");
        let canon: &[u8] = b"<html>\n<body>ru</body>\n</html>";
        let edt_bytes: &[u8] = b"<html>\r\n<body>ru</body>\r\n</html>";

        // --- EDT source (BOM-less, CRLF) → canon.
        let edt_dir = base.join("edt").join("CommonTemplates").join("И");
        std::fs::create_dir_all(&edt_dir).unwrap();
        crate::fsio::write(edt_dir.join("ru.html"), edt_bytes).unwrap();
        crate::fsio::write(
            edt_dir.join(EDT_HTMLDOC_FILE),
            edt_htmldoc_manifest(&["ru".to_string()]),
        )
        .unwrap();
        let mut obj = tmpl("CommonTemplate", "HTMLDocument", "И");
        attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir.join("И.mdo"),
            &mut obj,
        )
        .unwrap();
        assert_eq!(
            obj.templates[0].pages,
            vec![("ru".to_string(), canon.to_vec())],
            "canon = LF, BOM-less"
        );
        assert_eq!(obj.templates[0].body, None, "HTML pages live in `pages`, not `body`");

        // --- canon → Designer (BOM + LF page under `Ext/Template/`, `Ext/Template.xml` manifest).
        let des_root = base.join("des").join("CommonTemplates");
        std::fs::create_dir_all(&des_root).unwrap();
        let des_desc = des_root.join("И.xml");
        write_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &obj).unwrap();
        let ext = des_root.join("И").join("Ext");
        let page = std::fs::read(ext.join(DESIGNER_HTML_DIR).join("ru.html")).unwrap();
        assert!(page.starts_with(BOM), "Designer page carries a BOM");
        assert_eq!(&page[3..], canon, "Designer page keeps LF");
        assert_eq!(
            std::fs::read(ext.join(DESIGNER_HTMLDOC_FILE)).unwrap(),
            crate::help_read::serialize_page_descriptor(&["ru"])
        );

        // --- Designer → canon → EDT (back to BOM-less CRLF + the g5 manifest), byte-exact.
        let mut back = tmpl("CommonTemplate", "HTMLDocument", "И");
        attach_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &mut back)
            .unwrap();
        assert_eq!(back.templates[0].pages, vec![("ru".to_string(), canon.to_vec())]);

        let edt2 = base.join("edt2").join("CommonTemplates").join("И");
        std::fs::create_dir_all(&edt2).unwrap();
        write_blob_template_bodies(Format::Edt, "CommonTemplate", &edt2.join("И.mdo"), &back)
            .unwrap();
        assert_eq!(
            std::fs::read(edt2.join("ru.html")).unwrap(),
            edt_bytes,
            "EDT page re-encoded to CRLF, BOM-less (byte-exact with the source)"
        );
        assert_eq!(
            std::fs::read(edt2.join(EDT_HTMLDOC_FILE)).unwrap(),
            edt_htmldoc_manifest(&["ru".to_string()]).as_bytes()
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Multi-page HTMLDocument (ERP-witnessed 44×`(en,ru)`): both pages attach in manifest
    /// order and re-emit with the multi-lang manifests in both dialects.
    #[test]
    fn html_multi_page_round_trips() {
        let base = scratch("html-multi");
        let en: &[u8] = b"<html>en</html>";
        let ru: &[u8] = b"<html>ru</html>";

        let edt_dir = base.join("edt").join("CommonTemplates").join("И");
        std::fs::create_dir_all(&edt_dir).unwrap();
        crate::fsio::write(edt_dir.join("en.html"), en).unwrap();
        crate::fsio::write(edt_dir.join("ru.html"), ru).unwrap();
        crate::fsio::write(
            edt_dir.join(EDT_HTMLDOC_FILE),
            edt_htmldoc_manifest(&["en".to_string(), "ru".to_string()]),
        )
        .unwrap();
        let mut obj = tmpl("CommonTemplate", "HTMLDocument", "И");
        attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir.join("И.mdo"),
            &mut obj,
        )
        .unwrap();
        assert_eq!(
            obj.templates[0].pages,
            vec![
                ("en".to_string(), en.to_vec()),
                ("ru".to_string(), ru.to_vec())
            ]
        );

        // → Designer: two pages + the two-lang `<Help>` manifest.
        let des_root = base.join("des").join("CommonTemplates");
        std::fs::create_dir_all(&des_root).unwrap();
        let des_desc = des_root.join("И.xml");
        write_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &obj).unwrap();
        let ext = des_root.join("И").join("Ext");
        assert!(ext.join(DESIGNER_HTML_DIR).join("en.html").is_file());
        assert!(ext.join(DESIGNER_HTML_DIR).join("ru.html").is_file());
        assert_eq!(
            std::fs::read(ext.join(DESIGNER_HTMLDOC_FILE)).unwrap(),
            crate::help_read::serialize_page_descriptor(&["en", "ru"])
        );

        // Designer → IR: same pages, manifest order preserved.
        let mut back = tmpl("CommonTemplate", "HTMLDocument", "И");
        attach_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &mut back)
            .unwrap();
        assert_eq!(back.templates[0].pages, obj.templates[0].pages);

        let _ = std::fs::remove_dir_all(&base);
    }

    /// `_files/`-ресурсы HTMLDocument-макета (ERP-witnessed 14 носителей): читаются в
    /// `Template::resources` сортированным обходом (== cf-хвост), переносятся между
    /// диалектами вербатим (EDT `<dir>/_files/…` ↔ Designer `Ext/Template/_files/…`).
    #[test]
    fn html_files_resources_round_trip_both_dialects() {
        let base = scratch("html-files");
        let html: &[u8] = b"<img src=\"_files/a.png\">";
        let png_a: &[u8] = b"\x89PNG\r\n\x1a\nAAAA";
        let png_b: &[u8] = b"\x89PNG\r\n\x1a\nBBBB";

        // EDT source: pages + manifest + `_files/` siblings.
        let edt_dir = base.join("edt").join("CommonTemplates").join("И");
        std::fs::create_dir_all(edt_dir.join(FILES_DIR)).unwrap();
        crate::fsio::write(edt_dir.join("ru.html"), html).unwrap();
        crate::fsio::write(
            edt_dir.join(EDT_HTMLDOC_FILE),
            edt_htmldoc_manifest(&["ru".to_string()]),
        )
        .unwrap();
        // Пишем НЕ в сортированном порядке — ридер обязан отсортировать.
        crate::fsio::write(edt_dir.join(FILES_DIR).join("b.png"), png_b).unwrap();
        crate::fsio::write(edt_dir.join(FILES_DIR).join("a.png"), png_a).unwrap();
        let mut obj = tmpl("CommonTemplate", "HTMLDocument", "И");
        attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir.join("И.mdo"),
            &mut obj,
        )
        .unwrap();
        assert_eq!(
            obj.templates[0].resources,
            vec![
                ("a.png".to_string(), png_a.to_vec()),
                ("b.png".to_string(), png_b.to_vec())
            ],
            "resources attach sorted, raw verbatim"
        );

        // → Designer: `Ext/Template/_files/<имя>` byte-verbatim.
        let des_root = base.join("des").join("CommonTemplates");
        std::fs::create_dir_all(&des_root).unwrap();
        let des_desc = des_root.join("И.xml");
        write_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &obj).unwrap();
        let des_files = des_root
            .join("И")
            .join("Ext")
            .join(DESIGNER_HTML_DIR)
            .join(FILES_DIR);
        assert_eq!(std::fs::read(des_files.join("a.png")).unwrap(), png_a);
        assert_eq!(std::fs::read(des_files.join("b.png")).unwrap(), png_b);

        // Designer → canon: same resources; page src stays bare `_files/` (dialect verbatim).
        let mut back = tmpl("CommonTemplate", "HTMLDocument", "И");
        attach_blob_template_bodies(Format::Designer, "CommonTemplate", &des_desc, &mut back)
            .unwrap();
        assert_eq!(back.templates[0].resources, obj.templates[0].resources);
        assert_eq!(back.templates[0].pages, obj.templates[0].pages);

        // → EDT: `_files/` siblings restored verbatim.
        let edt2 = base.join("edt2").join("CommonTemplates").join("И");
        std::fs::create_dir_all(&edt2).unwrap();
        write_blob_template_bodies(Format::Edt, "CommonTemplate", &edt2.join("И.mdo"), &back)
            .unwrap();
        assert_eq!(std::fs::read(edt2.join(FILES_DIR).join("a.png")).unwrap(), png_a);
        assert_eq!(std::fs::read(edt2.join(FILES_DIR).join("b.png")).unwrap(), png_b);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn refuses_incoherent_html_sidecars() {
        let base = scratch("html-bad");

        // EDT: a malformed manifest (pages without lang) is a LOUD read error.
        let edt_dir = base.join("edt").join("CommonTemplates").join("И");
        std::fs::create_dir_all(&edt_dir).unwrap();
        crate::fsio::write(edt_dir.join("ru.html"), b"<html/>").unwrap();
        crate::fsio::write(
            edt_dir.join(EDT_HTMLDOC_FILE),
            b"<htmldoc:HtmlDocument><pages/></htmldoc:HtmlDocument>",
        )
        .unwrap();
        let mut obj = tmpl("CommonTemplate", "HTMLDocument", "И");
        let err = attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir.join("И.mdo"),
            &mut obj,
        )
        .unwrap_err();
        assert!(err.to_string().contains("malformed"), "EDT: {err}");

        // EDT: a declared page whose file is missing is incoherent.
        let edt_dir2 = base.join("edt2").join("CommonTemplates").join("И");
        std::fs::create_dir_all(&edt_dir2).unwrap();
        crate::fsio::write(
            edt_dir2.join(EDT_HTMLDOC_FILE),
            edt_htmldoc_manifest(&["en".to_string(), "ru".to_string()]),
        )
        .unwrap();
        crate::fsio::write(edt_dir2.join("en.html"), b"<html/>").unwrap();
        let mut obj2 = tmpl("CommonTemplate", "HTMLDocument", "И");
        let err2 = attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir2.join("И.mdo"),
            &mut obj2,
        )
        .unwrap_err();
        assert!(err2.to_string().contains("incoherent"), "EDT: {err2}");

        // Designer: an orphan page without a manifest is incoherent.
        let des_root = base.join("des").join("CommonTemplates");
        let ext = des_root.join("И").join("Ext");
        std::fs::create_dir_all(ext.join(DESIGNER_HTML_DIR)).unwrap();
        crate::fsio::write(ext.join(DESIGNER_HTML_DIR).join("ru.html"), b"<html/>").unwrap();
        let mut obj3 = tmpl("CommonTemplate", "HTMLDocument", "И");
        let err3 = attach_blob_template_bodies(
            Format::Designer,
            "CommonTemplate",
            &des_root.join("И.xml"),
            &mut obj3,
        )
        .unwrap_err();
        assert!(err3.to_string().contains("incoherent"), "Designer: {err3}");

        // EDT: an orphan `_files/` dir without a manifest is incoherent too (a silent skip
        // would drop the resources on the floor — §1.0).
        let edt_dir3 = base.join("edt3").join("CommonTemplates").join("И");
        std::fs::create_dir_all(edt_dir3.join(FILES_DIR)).unwrap();
        crate::fsio::write(edt_dir3.join(FILES_DIR).join("a.png"), b"\x89PNG").unwrap();
        let mut obj4 = tmpl("CommonTemplate", "HTMLDocument", "И");
        let err4 = attach_blob_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &edt_dir3.join("И.mdo"),
            &mut obj4,
        )
        .unwrap_err();
        assert!(err4.to_string().contains("incoherent"), "EDT _files: {err4}");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attaches_child_blob_bodies_in_both_dialects() {
        let base = scratch("child");

        // EDT: `<host-dir>/Templates/<T>/Template.bin`.
        let tdir = base
            .join("edt")
            .join("Catalogs")
            .join("Спр")
            .join("Templates")
            .join("Ограничения");
        std::fs::create_dir_all(&tdir).unwrap();
        crate::fsio::write(tdir.join(BIN_FILE), b"\x01\x02binary").unwrap();
        let host = base
            .join("edt")
            .join("Catalogs")
            .join("Спр")
            .join("Спр.mdo");

        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([2; 16]));
        obj.children
            .push(tmpl("Catalog.TemplateRef", "BinaryData", "Ограничения"));
        attach_blob_template_bodies(Format::Edt, "Catalog", &host, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].templates[0].body.as_deref(),
            Some(b"\x01\x02binary".as_slice())
        );

        // Designer: `<host>/Templates/<T>/Ext/Template.bin` — same bytes.
        let des_cat = base.join("des").join("Catalogs");
        std::fs::create_dir_all(&des_cat).unwrap();
        let des_host = des_cat.join("Спр.xml");
        write_blob_template_bodies(Format::Designer, "Catalog", &des_host, &obj).unwrap();
        assert_eq!(
            std::fs::read(
                des_cat
                    .join("Спр")
                    .join("Templates")
                    .join("Ограничения")
                    .join("Ext")
                    .join(BIN_FILE)
            )
            .unwrap(),
            b"\x01\x02binary"
        );

        let mut back = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([2; 16]));
        back.children
            .push(tmpl("Catalog.TemplateRef", "BinaryData", "Ограничения"));
        attach_blob_template_bodies(Format::Designer, "Catalog", &des_host, &mut back).unwrap();
        assert_eq!(
            back.children[0].templates[0].body.as_deref(),
            Some(b"\x01\x02binary".as_slice())
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
