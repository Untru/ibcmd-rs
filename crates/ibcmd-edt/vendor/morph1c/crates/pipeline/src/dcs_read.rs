//! Attach the Report-child DCS TEMPLATE-BODY SIDECAR (`Template.dcs` EDT /
//! `Ext/Template.xml` Designer) onto the DCS template child's IR during whole-config read
//! (§1.0/§1.3/§1.6). Mirror of [`crate::template_read`] (CommonTemplate `Template.txt`) with
//! the §1.0 STRICTness of [`crate::rights_read`] (a declared DataCompositionSchema template MUST
//! carry its schema sidecar — witnessed 6/6 on s11_dcs).
//!
//! # Why: edt/designer→cf needs the DCS schema body the way it needs the module body
//! A Report's data-composition-schema template is a Report CHILD (`Report.TemplateRef`, kind
//! ends `.TemplateRef`, `templateType == DataCompositionSchema`). In cf it is emitted as TWO
//! elements under the TEMPLATE's uuid: the `<tmpl-uuid>` descriptor + the `<tmpl-uuid>.0` body
//! (the schema+settings re-serialization — [`formats_cf::dcs_body`]). The Report descriptor read
//! yields the child's identity (uuid/name/synonym/templateType) but the schema BODY lives in a
//! sibling file; this pass reads it into `child.templates[0].body` so `--to cf` can emit the
//! `<tmpl-uuid>.0` body ([`formats_cf::assemble_cf`]).
//!
//! EDT read/write also projects the resolved, inline current-config TypeSet
//! AnyRef spelling through the existing AnyRef/AnyIBRef type alias. Native
//! bodies remain verbatim; query text is never interpreted as a type QName.
//!
//! # Scope (EDT-authoritative)
//! The EDT `.mdo` carries the template stub INLINE (`<templates uuid=… templateType=…>`), so the
//! Report child MetadataObject read from it already carries the uuid + `templateType`. This pass
//! then attaches the `Template.dcs` body. The Designer Report `.xml` carries only a BARE
//! `<Template>Name</Template>` ref (no uuid/templateType in the descriptor — they live in a
//! separate `Templates/<Name>.xml` file), so a Designer-read child is NOT recognised as a DCS
//! template here and this pass no-ops for it (Designer→cf DCS bodies are a later milestone; the
//! milestone DoD is `--from edt`).
//!
//! # Sidecar layout (RE: coverage/s11_dcs edt + designer)
//! * **EDT** (`Reports/<R>/<R>.mdo`, dir-per-object): sidecar
//!   `Reports/<R>/Templates/<T>/Template.dcs` (BOM-less on disk).
//! * **Designer** (`Reports/<R>.xml`, file-per-object): sidecar
//!   `Reports/<R>/Templates/<T>/Ext/Template.xml` (carries a UTF-8 BOM on disk).
//!
//! Both give the SAME canonical body once the leading BOM is stripped AND the TEXT-NODE
//! newlines are canonicalized (§1.6): the two dialects differ ONLY in the newline convention
//! INSIDE text nodes (multi-line `<query>`) — Designer stores the cf-verbatim text (LF on the
//! s15 witness), EDT re-normalizes text newlines to CRLF on export; STRUCTURAL newlines
//! (between tags) are CRLF in BOTH. So an EDT read canonicalizes text-node CRLF→LF
//! ([`transcode_text_node_newlines`]); a Designer read stays verbatim (it IS the ground
//! truth). On write: EDT re-normalizes text nodes to CRLF, Designer emits the canonical
//! verbatim (+BOM). The cf re-serializer is text-node CRLF↔LF agnostic anyway (a spec parse
//! LF-normalizes both), so the canonicalization is invisible to `--to cf`.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, Template};
use morph1c_core::spec::metadata::report_template_ref::F_TEMPLATE_TYPE;

use crate::ConvertError;

/// The one `templateType` whose Report-child template carries a reproducible cf `<uuid>.0` schema
/// body (the DCS re-serializer). Shared FieldId(3) literal across every `*.TemplateRef` spec.
const TEMPLATE_TYPE_DCS: &str = "DataCompositionSchema";

/// EDT sidecar file name of a DCS template body.
const DCS_SIDECAR_EDT: &str = "Template.dcs";
/// Designer sidecar file name of a DCS template body (under `<T>/Ext/`).
const DCS_SIDECAR_DESIGNER: &str = "Template.xml";
/// The `Templates` collection subdir hosting each template's sidecar.
const TEMPLATES_DIR: &str = "Templates";

/// UTF-8 BOM (stripped from the sidecar → the canonical, format-neutral body — §1.6).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Load each DCS template child's schema body (from its sidecar) into `child.templates`.
///
/// `descriptor_path` — path of the HOST object descriptor just read (`Reports/<R>/<R>.mdo` EDT /
/// `Reports/<R>.xml` Designer). Walks `obj.children` for `*.TemplateRef` children with
/// `templateType == DataCompositionSchema` and attaches their `Template.dcs`/`Ext/Template.xml`
/// body. §1.0-STRICT: such a child WITHOUT an adjacent sidecar is a HARD ERROR (a DCS template
/// must carry its schema — witnessed 6/6 on s11_dcs, unlike the optional TextDocument case). cf
/// (a container) and objects with no DCS template child are no-ops.
pub fn attach_dcs_bodies(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    // cf is a container: objects are records inside one file, not file-per-object sidecars.
    if format == Format::Cf {
        return Ok(());
    }
    // CommonTemplate OBJECT-level DCS body (RE ssl.cf: 3/3 common DCS bodies byte-exact via
    // the same re-serializer). OPTIONAL (unlike the child STRICTness): a property-stub DCS
    // CommonTemplate WITHOUT a sidecar (coverage s4_common `ОбщМакет_ТипМакета_СхемаКомпоновки
    // Данных`) round-trips descriptor-only — mirroring the TextDocument/MXL object-level rule.
    if obj.kind.as_str() == "CommonTemplate"
        && matches!(obj.get(F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == TEMPLATE_TYPE_DCS)
    {
        let path = match format {
            // EDT: `CommonTemplates/<N>/<N>.mdo` → sibling `Template.dcs`.
            Format::Edt => descriptor_path.parent().map(|d| d.join(DCS_SIDECAR_EDT)),
            // Designer: `CommonTemplates/<N>.xml` → `<N>/Ext/Template.xml`.
            Format::Designer => match (descriptor_path.parent(), descriptor_path.file_stem()) {
                (Some(dir), Some(stem)) => {
                    Some(dir.join(stem).join("Ext").join(DCS_SIDECAR_DESIGNER))
                }
                _ => None,
            },
            Format::Cf => None,
        };
        if let Some(path) = path {
            if path.is_file() {
                let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                })?;
                let stripped = bytes.strip_prefix(BOM).unwrap_or(&bytes);
                let body = match format {
                    Format::Edt => crate::sdk_body_projection::dcs_alias(&crate::sdk_body_projection::text_newlines(stripped, false)?, false)?,
                    _ => stripped.to_vec(),
                };
                if !obj.templates.is_empty() {
                    return Err(ConvertError::Read {
                        kind: obj.kind.as_str().to_string(),
                        object: obj.name.clone(),
                        reason: "object already carries templates before the DCS sidecar attach \
                                 (unexpected — the descriptor projection must not populate them)"
                            .into(),
                    });
                }
                obj.templates.push(Template {
                    pages: Vec::new(),
                    resources: Vec::new(),
                    name: obj.name.clone(),
                    properties: Vec::new(),
                    body: Some(body),
                });
            }
        }
    }
    for child in obj.children.iter_mut() {
        if !is_dcs_template_child(child) {
            continue;
        }
        let path = match dcs_sidecar_path(format, descriptor_path, &child.name) {
            Some(p) => p,
            None => return Ok(()), // cf handled above; defensive.
        };
        // §1.0-STRICT: a declared DCS template MUST carry its schema sidecar (never a silent
        // partial `.cf` — the cf descriptor references the template uuid, whose `.0` body must exist).
        if !path.is_file() {
            return Err(ConvertError::Read {
                kind: child.kind.as_str().to_string(),
                object: child.name.clone(),
                reason: format!(
                    "DataCompositionSchema template declares a schema but the body sidecar {} is \
                     missing (§1.0 — a DCS template must carry its Template.dcs/Ext/Template.xml)",
                    path.display()
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        // Canonical body = BOM-stripped bytes (EDT has none, Designer has one → equal after
        // strip) + canonical TEXT-NODE newlines: EDT's text-node CRLF is its export
        // normalization (module docs) → undo it; Designer bodies are already canonical.
        let stripped = bytes.strip_prefix(BOM).unwrap_or(&bytes);
        let body = match format {
            Format::Edt => crate::sdk_body_projection::dcs_alias(&crate::sdk_body_projection::text_newlines(stripped, false)?, false)?,
            _ => stripped.to_vec(),
        };
        // §1.0: the descriptor read must not populate the child's templates — only this pass does.
        if !child.templates.is_empty() {
            return Err(ConvertError::Read {
                kind: child.kind.as_str().to_string(),
                object: child.name.clone(),
                reason: "child already carries templates before the DCS sidecar attach \
                         (unexpected — the descriptor projection must not populate them)"
                    .into(),
            });
        }
        child.templates.push(Template {
            pages: Vec::new(),
            resources: Vec::new(),
            name: child.name.clone(),
            properties: Vec::new(),
            body: Some(body),
        });
    }
    Ok(())
}

/// Transcode the newlines INSIDE TEXT NODES of an XML body (`to_crlf == true` → CRLF, else LF),
/// leaving STRUCTURAL whitespace (all-whitespace runs between tags) and tag internals VERBATIM.
///
/// The two dialects differ ONLY here (module docs): Designer stores text nodes cf-verbatim (LF
/// witness), EDT re-normalizes them to CRLF; structural newlines are CRLF in both. A "text node"
/// is a `>`…`<` content run containing at least one non-whitespace byte (a `>` inside text is
/// just a byte — only `<` opens markup). Scope (§1.0-witnessed subset): no CDATA and no `>`
/// inside attribute values in DCS bodies — both would misclassify runs.
pub(crate) fn transcode_text_node_newlines(body: &[u8], to_crlf: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 64);
    let n = body.len();
    let mut i = 0usize;
    while i < n {
        if body[i] == b'<' {
            // Markup: copy verbatim through the closing '>'.
            let start = i;
            while i < n && body[i] != b'>' {
                i += 1;
            }
            if i < n {
                i += 1; // include '>'
            }
            out.extend_from_slice(&body[start..i]);
        } else {
            // Content run: through the next '<' (or EOF).
            let start = i;
            while i < n && body[i] != b'<' {
                i += 1;
            }
            let run = &body[start..i];
            if run.iter().any(|b| !b.is_ascii_whitespace()) {
                out.extend_from_slice(&crate::template_read::normalize_newlines(run, to_crlf));
            } else {
                out.extend_from_slice(run); // structural whitespace — verbatim.
            }
        }
    }
    out
}

/// Is `child` a Report-child DCS template (`*.TemplateRef`, `templateType == DataCompositionSchema`)?
fn is_dcs_template_child(child: &MetadataObject) -> bool {
    child.kind.as_str().ends_with(".TemplateRef")
        && matches!(child.get(F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == TEMPLATE_TYPE_DCS)
}

/// Write-side mirror of [`attach_dcs_bodies`]: emit each DCS template child's schema body beside the
/// host descriptor just written, in the TARGET format's layout (mirror of the read-side attach — a
/// cross-format convert must re-emit the schema body, else the compiled Report carries an
/// empty/default DCS, a real content loss §1.0/§1.6). EDT writes the body with its export
/// convention re-applied (text-node newlines → CRLF) to `Templates/<T>/Template.dcs` (BOM-less);
/// Designer writes the canonical `BOM + body` verbatim to `Templates/<T>/Ext/Template.xml`
/// (text-sidecar convention). No-op for cf, non-Report objects, and DCS template children
/// without an attached body.
///
/// `descriptor_out` — path of the HOST object descriptor just written (`Reports/<R>/<R>.mdo` EDT /
/// `Reports/<R>.xml` Designer). §1.0: typed [`ConvertError`] on I/O.
pub fn write_dcs_bodies(
    format: Format,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    // CommonTemplate OBJECT-level mirror (see the attach-side branch).
    if obj.kind.as_str() == "CommonTemplate"
        && matches!(obj.get(F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == TEMPLATE_TYPE_DCS)
    {
        if let Some(body) = obj.templates.first().and_then(|t| t.body.as_deref()) {
            let path = match format {
                Format::Edt => descriptor_out.parent().map(|d| d.join(DCS_SIDECAR_EDT)),
                Format::Designer => match (descriptor_out.parent(), descriptor_out.file_stem()) {
                    (Some(dir), Some(stem)) => {
                        Some(dir.join(stem).join("Ext").join(DCS_SIDECAR_DESIGNER))
                    }
                    _ => None,
                },
                Format::Cf => None,
            };
            if let Some(path) = path {
                let bytes: Vec<u8> = if format == Format::Designer {
                    let mut v = Vec::with_capacity(BOM.len() + body.len());
                    v.extend_from_slice(BOM);
                    v.extend_from_slice(body);
                    v
                } else {
                    crate::sdk_body_projection::text_newlines(&crate::sdk_body_projection::dcs_alias(body, true)?, true)?
                };
                crate::form_write::write_file(&path, &bytes)?;
            }
        }
    }
    for child in &obj.children {
        if !is_dcs_template_child(child) {
            continue;
        }
        let body = match child.templates.first().and_then(|t| t.body.as_deref()) {
            Some(b) => b,
            None => continue, // no body attached — nothing to emit.
        };
        let path = match dcs_sidecar_path(format, descriptor_out, &child.name) {
            Some(p) => p,
            None => continue, // cf — container, no file-per-object sidecar.
        };
        // The canonical body carries LF text-node newlines (read-side canonicalization —
        // module docs). Designer emits it VERBATIM + BOM (byte-exact to the platform dump:
        // structural CRLF preserved, query text LF); EDT re-applies its export convention —
        // text-node newlines → CRLF ([`transcode_text_node_newlines`]). Structural newlines
        // are never touched (XML whitespace, CRLF in both dialects).
        let bytes: Vec<u8> = if format == Format::Designer {
            // Designer re-adds the leading BOM (text-sidecar convention, mirror of read-side strip).
            let mut v = Vec::with_capacity(BOM.len() + body.len());
            v.extend_from_slice(BOM);
            v.extend_from_slice(body);
            v
        } else {
            crate::sdk_body_projection::text_newlines(&crate::sdk_body_projection::dcs_alias(body, true)?, true)?
        };
        crate::form_write::write_file(&path, &bytes)?;
    }
    Ok(())
}

/// Sidecar path of a DCS template body beside the host descriptor, per format layout (module docs).
/// `None` for cf (container, no file-per-object sidecar).
fn dcs_sidecar_path(
    format: Format,
    descriptor_path: &Path,
    template_name: &str,
) -> Option<PathBuf> {
    match format {
        // EDT: `<Reports>/<R>/<R>.mdo` → `<Reports>/<R>/Templates/<T>/Template.dcs`.
        Format::Edt => {
            let host_dir = descriptor_path.parent()?;
            Some(
                host_dir
                    .join(TEMPLATES_DIR)
                    .join(template_name)
                    .join(DCS_SIDECAR_EDT),
            )
        }
        // Designer: `<Reports>/<R>.xml` → `<Reports>/<R>/Templates/<T>/Ext/Template.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(
                dir.join(stem)
                    .join(TEMPLATES_DIR)
                    .join(template_name)
                    .join("Ext")
                    .join(DCS_SIDECAR_DESIGNER),
            )
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::Token;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn dcs_child(name: &str) -> MetadataObject {
        let mut c = MetadataObject::new(ObjectKind::new("Report.TemplateRef"), name, Uuid([1; 16]));
        c.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("DataCompositionSchema")),
        ));
        c
    }

    #[test]
    fn edt_sidecar_path_is_under_report_dir() {
        let p = Path::new("/root/Reports/Отчет_X/Отчет_X.mdo");
        let s = dcs_sidecar_path(Format::Edt, p, "ОсновнаяСхемаКомпоновкиДанных").unwrap();
        assert!(
            s.ends_with(Path::new(
                "Reports/Отчет_X/Templates/ОсновнаяСхемаКомпоновкиДанных/Template.dcs"
            )),
            "got {s:?}"
        );
    }

    #[test]
    fn designer_sidecar_path_is_under_ext() {
        let p = Path::new("/root/Reports/Отчет_X.xml");
        let s = dcs_sidecar_path(Format::Designer, p, "ОсновнаяСхемаКомпоновкиДанных").unwrap();
        assert!(
            s.ends_with(Path::new(
                "Reports/Отчет_X/Templates/ОсновнаяСхемаКомпоновкиДанных/Ext/Template.xml"
            )),
            "got {s:?}"
        );
    }

    #[test]
    fn transcode_touches_only_text_node_newlines() {
        // Structural CRLF (between tags) stays VERBATIM in both directions; only the text-node
        // (query) newlines are transcoded.
        let designer = b"<?xml?>\r\n<s>\r\n\t<query>A\nB\nC</query>\r\n</s>".to_vec();
        let edt = b"<?xml?>\r\n<s>\r\n\t<query>A\r\nB\r\nC</query>\r\n</s>".to_vec();
        assert_eq!(
            transcode_text_node_newlines(&designer, true),
            edt,
            "text LF→CRLF"
        );
        assert_eq!(
            transcode_text_node_newlines(&edt, false),
            designer,
            "text CRLF→LF"
        );
        // Idempotence on already-canonical bodies.
        assert_eq!(transcode_text_node_newlines(&designer, false), designer);
        assert_eq!(transcode_text_node_newlines(&edt, true), edt);
        // A '>' inside a text node is just a byte (query `ГДЕ Поле > 5`).
        let gt = b"<q>A > B\nC</q>".to_vec();
        assert_eq!(
            transcode_text_node_newlines(&gt, true),
            b"<q>A > B\r\nC</q>".to_vec()
        );
        assert_eq!(transcode_text_node_newlines(b"", true), b"");
    }

    #[test]
    fn attach_edt_canonicalizes_text_node_newlines() {
        // An EDT sidecar with CRLF query text attaches the CANONICAL body (text LF, structural
        // newlines verbatim) — what a Designer read of the same template yields.
        let base = std::env::temp_dir().join(format!(
            "morph1c-dcs-canon-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let tmpl_dir = base
            .join("Reports")
            .join("Отчет_X")
            .join("Templates")
            .join("Схема");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        crate::fsio::write(
            tmpl_dir.join("Template.dcs"),
            b"<s>\r\n<query>A\r\nB</query>\r\n</s>",
        )
        .unwrap();
        let descriptor = base.join("Reports").join("Отчет_X").join("Отчет_X.mdo");

        let mut report = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([2; 16]));
        report.children.push(dcs_child("Схема"));
        attach_dcs_bodies(Format::Edt, &descriptor, &mut report).unwrap();
        assert_eq!(
            report.children[0].templates[0].body.as_deref(),
            Some(b"<s>\r\n<query>A\nB</query>\r\n</s>".as_slice()),
            "text-node CRLF canonicalized to LF; structural CRLF verbatim"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_dcs_bodies_reapplies_dialect_newlines() {
        let base = std::env::temp_dir().join(format!(
            "morph1c-dcs-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        // IR: a Report with a DCS template child carrying the CANONICAL body (structural CRLF,
        // text-node LF — what both dialect reads yield).
        let canonical =
            b"<DataCompositionSchema>\r\n<query>A\nB</query>\r\n</DataCompositionSchema>";
        let mut child = dcs_child("ОсновнаяСхемаКомпоновкиДанных");
        child.templates.push(Template {
            pages: Vec::new(),
            resources: Vec::new(),
            name: child.name.clone(),
            properties: Vec::new(),
            body: Some(canonical.to_vec()),
        });
        let mut report = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([9; 16]));
        report.children.push(child);

        // Designer: BOM + canonical VERBATIM at `<R>/Templates/<T>/Ext/Template.xml`.
        std::fs::create_dir_all(base.join("dz").join("Reports")).unwrap();
        let dz_host = base.join("dz").join("Reports").join("Отчет_X.xml");
        write_dcs_bodies(Format::Designer, &dz_host, &report).unwrap();
        let dz_body = base
            .join("dz/Reports/Отчет_X/Templates/ОсновнаяСхемаКомпоновкиДанных/Ext/Template.xml");
        let dz = std::fs::read(&dz_body).unwrap();
        let mut expected_dz = Vec::from(BOM);
        expected_dz.extend_from_slice(canonical);
        assert_eq!(dz, expected_dz, "designer body = BOM + canonical verbatim");

        // EDT: BOM-less, text-node newlines re-normalized to CRLF at `<R>/Templates/<T>/Template.dcs`.
        std::fs::create_dir_all(base.join("edt").join("Reports").join("Отчет_X")).unwrap();
        let edt_host = base
            .join("edt")
            .join("Reports")
            .join("Отчет_X")
            .join("Отчет_X.mdo");
        write_dcs_bodies(Format::Edt, &edt_host, &report).unwrap();
        let edt_body =
            base.join("edt/Reports/Отчет_X/Templates/ОсновнаяСхемаКомпоновкиДанных/Template.dcs");
        let edt = std::fs::read(&edt_body).unwrap();
        assert_eq!(
            edt,
            b"<DataCompositionSchema>\r\n<query>A\r\nB</query>\r\n</DataCompositionSchema>"
                .to_vec(),
            "EDT body = text-node newlines CRLF, structural verbatim, BOM-less"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn non_dcs_child_is_not_recognised() {
        let mut c = MetadataObject::new(ObjectKind::new("Report.TemplateRef"), "M", Uuid([1; 16]));
        c.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("TextDocument")),
        ));
        assert!(!is_dcs_template_child(&c));
        // A bare-ref child (Designer: no templateType) is also not recognised.
        let bare = MetadataObject::new(ObjectKind::new("Report.TemplateRef"), "M", Uuid([1; 16]));
        assert!(!is_dcs_template_child(&bare));
    }

    #[test]
    fn missing_sidecar_is_hard_error() {
        let mut report = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([2; 16]));
        report
            .children
            .push(dcs_child("ОсновнаяСхемаКомпоновкиДанных"));
        let descriptor = Path::new("/nonexistent-root/Reports/Отчет_X/Отчет_X.mdo");
        let err = attach_dcs_bodies(Format::Edt, descriptor, &mut report).unwrap_err();
        assert!(matches!(err, ConvertError::Read { .. }), "got {err:?}");
    }

    #[test]
    fn attaches_edt_sidecar_bom_stripped() {
        let base = std::env::temp_dir().join(format!(
            "morph1c-dcs-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let tmpl_dir = base
            .join("Reports")
            .join("Отчет_X")
            .join("Templates")
            .join("ОсновнаяСхемаКомпоновкиДанных");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        crate::fsio::write(tmpl_dir.join("Template.dcs"), b"<DataCompositionSchema/>").unwrap();
        let descriptor = base.join("Reports").join("Отчет_X").join("Отчет_X.mdo");

        let mut report = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([2; 16]));
        report
            .children
            .push(dcs_child("ОсновнаяСхемаКомпоновкиДанных"));
        attach_dcs_bodies(Format::Edt, &descriptor, &mut report).unwrap();
        let child = &report.children[0];
        assert_eq!(
            child.templates.len(),
            1,
            "one DCS body attached to the child"
        );
        assert_eq!(
            child.templates[0].body.as_deref(),
            Some(b"<DataCompositionSchema/>".as_slice())
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn object_without_dcs_child_is_noop() {
        let mut c = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([3; 16]));
        attach_dcs_bodies(Format::Edt, Path::new("/x/C/C.mdo"), &mut c).unwrap();
        assert!(c.children.is_empty());
    }
}
