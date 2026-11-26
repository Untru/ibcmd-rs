//! Attach the VERBATIM-bodied TEMPLATE SIDECARS — TextDocument (`Template.txt` EDT /
//! `Ext/Template.txt` Designer) and DataCompositionAppearanceTemplate (`Template.dcsat` EDT /
//! `Ext/Template.xml` Designer) — into the IR `templates` during whole-config read, and
//! re-emit them on write (§1.0/§1.3/§1.6) — for the CommonTemplate OBJECT and for
//! OBJECT-SUBORDINATE template children (`*.TemplateRef` — Catalog/Document/…/DataProcessor
//! `Templates/<Name>`). Mirror of [`crate::module_read`] (CommonModule `Module.bsl`) — "the
//! descriptor read is metadata-only, the template BODY is a sibling file the pipeline attaches".
//!
//! # DataCompositionAppearanceTemplate (макет оформления КД) — VERBATIM like TextDocument
//! RE `.fixtures/ERP/cf/erp.cf` (5/5 DCAT CommonTemplates): cf `<uuid>.0` == Designer
//! `Ext/Template.xml` VERBATIM (BOM + XML) == `BOM + EDT Template.dcsat`. One canonical
//! (BOM-stripped) body, byte-identical across dialects — NO newline transcoding in either
//! direction (unlike the TextDocument CHILD lane below).
//!
//! # Why: edt/designer→cf needs the template body the way it needs the module body
//! The cf CommonTemplate writer emits the descriptor `<uuid>` PLUS a separate template-body
//! element `<uuid>.0` (`formats_cf::assemble_cf` TEMPLATE_BODY_KINDS → `formats_cf::template_body`).
//! That body is sourced from `MetadataObject.templates[0].body`. The per-kind descriptor connector
//! reads only the thin `.mdo`/`.xml` (synonym/comment/templateType); the template BODY lives in a
//! sidecar. This pass reads it into `obj.templates` so `--to cf` can emit the `<uuid>.0` body.
//!
//! # Scope: the VERBATIM types (TextDocument + DCAT) are this pass's; the rest re-encode
//! The cf `<uuid>.0` body form depends on `templateType` (see `formats_cf::template_body` survey):
//! * **TextDocument** (2/15 SSL) — cf body = `BOM + <text>` == a copy of the sidecar. RE byte-
//!   identical: `BOM + EDT Template.txt` == `Designer Ext/Template.txt` == cf `<uuid>.0` inflate.
//!   This pass reads that text (BOM-stripped → ONE canonical body, edt==designer §1.6) and attaches
//!   it. The body is OPTIONAL: a property-stub TextDocument descriptor with NO adjacent
//!   `Template.txt` (e.g. s4_common's `ОбщМакет_ТипМакета_ТекстовыйДокумент`) attaches nothing
//!   (reflects on-disk state) — never a hard error (an earlier STRICT rule broke s4 whole-config read).
//! * **DataCompositionAppearanceTemplate** (ERP 5/5) — same verbatim law with its own file
//!   names (`Template.dcsat` EDT / `Ext/Template.xml` Designer), see the section above.
//! * **SpreadsheetDocument/MXL, DataCompositionSchema, HTMLDocument, AddIn, BinaryData** —
//!   the cf body is a STRUCTURALLY-DIFFERENT re-encoding (binary MXL/DCS, multi-lang brace,
//!   base64-brace of the raw binary), NOT a copy of the EDT sidecar. NOT reproducible byte-exact
//!   as a blob → this pass does NOT attach a body (their own passes do: `mxl_read`/`dcs_read`/
//!   `geos_read`/`graph_template_read`/`blob_template_read`); an unencodable construct blocks
//!   LOUDLY at assemble (typed `CfError`/`BodyNotEmitted`) rather than fabricating a wrong
//!   `<uuid>.0` leaf (§1.0).
//!
//! # Sidecar layout beside the descriptor (RE: SSL/edt + SSL/designer_8.5.1/cf corpus)
//! * **EDT** (`CommonTemplates/<Name>/<Name>.mdo`, dir-per-object): sidecar SIBLING
//!   `CommonTemplates/<Name>/Template.txt` (BOM-less on disk).
//! * **Designer** (`CommonTemplates/<Name>.xml`, file-per-object): sidecar
//!   `CommonTemplates/<Name>/Ext/Template.txt` (carries a UTF-8 BOM on disk).
//!
//! Both give the SAME canonical body once the leading BOM is stripped (§1.6).
//!
//! # Object-subordinate templates (`*.TemplateRef` children — RE: coverage/s15_subordinate)
//! An OBJECT template (a Catalog's/Report's/DataProcessor's `Templates/<Name>`) is a CHILD of
//! its host (`kind` ends `.TemplateRef`). Its TextDocument body sidecar lives under the HOST:
//! * **EDT** (`<Kind>s/<Host>/<Host>.mdo`): `<Kind>s/<Host>/Templates/<Name>/Template.txt`.
//! * **Designer** (`<Kind>s/<Host>.xml`): `<Kind>s/<Host>/Templates/<Name>/Ext/Template.txt`.
//!
//! ## Child-body EOL is DATA — the read is VERBATIM in BOTH dialects (platform oracle)
//! The EOL of a TextDocument body is DATA, not envelope — the same law modules carry
//! (`formats_cf::module_body::encode_module_text`). PLATFORM ORACLE (r34): the LF-bodied
//! `coverage/cf/s15_subordinate.cf` dumped back with `DumpConfigToFiles` yields
//! `Ext/Template.txt` = `BOM + …LF` **17/17** — the platform preserves the body's newlines
//! EXACTLY in both directions (load AND dump), for LF and CRLF alike. Corpus (r34): on the
//! REAL dumps the two dialects' sidecars are byte-identical after BOM strip — SSL 31/31 and
//! ERP 854/854 (853 CRLF + 1 newline-less), ZERO mismatches. So the read must be VERBATIM in
//! both dialects, and it is: an EDT child read no longer canonicalizes CRLF→LF (that
//! normalization silently truncated 31 SSL bodies — 41 232 bytes over the whole config — and
//! wrote 31 bare-LF Designer sidecars on `edt→xml`).
//!
//! EDT is the lossy party, and only on WRITE: `1cedtcli` forces CRLF into `Template.txt` on its
//! own export (witnessed s15 17/17 — designer LF in, EDT CRLF out). Our EDT write mirrors that
//! convention (`write_text_body`), so `designer→edt` stays byte-equal to EDT's own projection;
//! the price is EDT's: an LF-bodied text template CANNOT be recovered from an EDT source (by us
//! or by EDT), so `edt→cf` of such a body legitimately yields CRLF. This is pinned as a
//! measured witness in `pipeline/tests/whole_s15_subordinate_cf_assemble.rs` (exactly 17
//! elements, each equal after CRLF→LF), never silently normalized away.
//!
//! The OBJECT-level (CommonTemplate) read was always verbatim — its cf `<uuid>.0` oracle is
//! `BOM + EDT Template.txt` byte-exact (SSL witness). Both levels now share ONE law.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, Template};
use morph1c_core::spec::metadata::common_template::F_TEMPLATE_TYPE;

use crate::ConvertError;

/// Kinds whose object carries a template-body sidecar (currently only `CommonTemplate`). Held
/// locally: extend as more template-bearing kinds gain a witnessed reproducible body layout.
const TEMPLATE_SIDECAR_KINDS: &[&str] = &["CommonTemplate"];

/// The one `templateType` whose cf `<uuid>.0` body == `BOM + sidecar` (byte-exact reproducible).
const TEMPLATE_TYPE_TEXT_DOCUMENT: &str = "TextDocument";

/// `templateType = DataCompositionAppearanceTemplate` (макет оформления компоновки данных) —
/// the SECOND verbatim type this pass owns: its cf `<uuid>.0` body == `BOM + sidecar XML`
/// VERBATIM (RE `.fixtures/ERP/cf/erp.cf` 5/5 CommonTemplates: `8ccba179…`/`3ebe4cdd…`/
/// `f49b7cf2…`/`1c59cb2f…`/`7ea366ad…` — byte-exact against BOTH dialects' sidecars).
const TEMPLATE_TYPE_DCAT: &str = "DataCompositionAppearanceTemplate";

/// The EDT/Designer sidecar file name of a TextDocument template body.
const TEXT_TEMPLATE_FILE: &str = "Template.txt";

/// The EDT sidecar file name of a DataCompositionAppearanceTemplate body (`Template.dcsat`,
/// BOM-less on disk). Designer stores it as `Ext/Template.xml` (with BOM) — the same file name
/// GeographicalSchema/DCS templates use, disambiguated by `templateType` (witnessed ERP 5/5).
const EDT_DCAT_TEMPLATE_FILE: &str = "Template.dcsat";
const DESIGNER_DCAT_TEMPLATE_FILE: &str = "Template.xml";

/// Per-format sidecar file names of a VERBATIM-bodied template type, or `None` when the type's
/// body is not this pass's (structural MXL/DCS/geo/graph/blob types have their own passes).
///
/// Both verbatim types share the canon law: canonical body = BOM-stripped sidecar bytes,
/// identical across dialects (§1.6 — witnessed SSL TextDocument 2/2, ERP DCAT 5/5).
fn verbatim_sidecar_files(ttype: &str) -> Option<(&'static str, &'static str)> {
    match ttype {
        TEMPLATE_TYPE_TEXT_DOCUMENT => Some((TEXT_TEMPLATE_FILE, TEXT_TEMPLATE_FILE)),
        TEMPLATE_TYPE_DCAT => Some((EDT_DCAT_TEMPLATE_FILE, DESIGNER_DCAT_TEMPLATE_FILE)),
        _ => None,
    }
}

/// UTF-8 BOM (stripped from the sidecar to get the canonical, format-neutral body — §1.6).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Is `child` an object-subordinate template stub (`kind` ends `.TemplateRef` —
/// Catalog/Document/…/DataProcessor `Templates/<Name>`)?
pub(crate) fn is_template_ref_child(child: &MetadataObject) -> bool {
    child.kind.as_str().ends_with(".TemplateRef")
}

/// `templateType` of a template descriptor/stub as `&str`, or the canonical default
/// `SpreadsheetDocument` when the (sparse) field is absent. Valid for BOTH the CommonTemplate
/// object and every `*.TemplateRef` child (the shared `FieldId(3)` literal across those specs).
pub(crate) fn template_type_of(obj: &MetadataObject) -> &str {
    match obj.get(F_TEMPLATE_TYPE) {
        Some(PropertyValue::Enum(t)) => t.as_str(),
        _ => "SpreadsheetDocument",
    }
}

/// Body-sidecar path of an object-subordinate template child under its HOST descriptor, per
/// format layout (module docs): EDT `<host-dir>/Templates/<T>/<edt_file>`, Designer
/// `<dir>/<host-stem>/Templates/<T>/Ext/<designer_file>`. `None` for cf (container).
pub(crate) fn child_sidecar_path(
    format: Format,
    host_descriptor: &Path,
    template_name: &str,
    edt_file: &str,
    designer_file: &str,
) -> Option<PathBuf> {
    const TEMPLATES_DIR: &str = "Templates";
    match format {
        Format::Edt => {
            let host_dir = host_descriptor.parent()?;
            Some(
                host_dir
                    .join(TEMPLATES_DIR)
                    .join(template_name)
                    .join(edt_file),
            )
        }
        Format::Designer => {
            let dir = host_descriptor.parent()?;
            let stem = host_descriptor.file_stem()?;
            Some(
                dir.join(stem)
                    .join(TEMPLATES_DIR)
                    .join(template_name)
                    .join("Ext")
                    .join(designer_file),
            )
        }
        Format::Cf => None,
    }
}

/// Normalize every newline of `body`: `to_crlf == true` → each `\n` / `\r\n` becomes `\r\n`;
/// `false` → each `\r\n` becomes `\n`. A lone `\r` (not a line ending) is left verbatim — only
/// the line-ending convention is transcoded, never data bytes.
pub(crate) fn normalize_newlines(body: &[u8], to_crlf: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 16);
    let mut i = 0;
    while i < body.len() {
        match body[i] {
            b'\r' if body.get(i + 1) == Some(&b'\n') => {
                out.extend_from_slice(if to_crlf { b"\r\n" } else { b"\n" });
                i += 2;
            }
            b'\n' => {
                out.extend_from_slice(if to_crlf { b"\r\n" } else { b"\n" });
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

/// Load the CommonTemplate body (from its sidecar) into `obj.templates`, AND each TextDocument
/// object-subordinate template child's body into `child.templates` (see module docs).
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). Only `TextDocument`
/// templates attach a body here. §1.0: the body is OPTIONAL — attach IFF the sidecar exists (a
/// bodyless property-stub TextDocument descriptor attaches nothing, reflecting the on-disk
/// state). Non-TextDocument template types attach NOTHING (their bodies belong to the
/// mxl/geos/dcs passes or are structural). Non-template kinds and cf (container) are no-ops.
/// NB (call-order): for Designer sources this MUST run AFTER
/// [`crate::template_ref_read::attach_designer_template_ref_descriptors`] — the bare-ref child
/// carries no `templateType` until that enrichment.
pub fn attach_template_body(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    attach_child_text_bodies(format, descriptor_path, obj)?;
    if !TEMPLATE_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    // Only the VERBATIM types (TextDocument / DataCompositionAppearanceTemplate) have a
    // byte-exact-reproducible cf body (= BOM + sidecar). Other types (MXL/DCS/HTML/AddIn/
    // BinaryData) are structurally re-encoded in cf → not a blob copy; leave `templates` empty
    // so the cf assembler blocks loudly (§1.0), never fabricating a wrong body. This is a
    // documented scope, not a silent skip (assemble surfaces BodyNotEmitted).
    let (edt_file, designer_file) = match verbatim_sidecar_files(template_type_of(obj)) {
        Some(files) => files,
        None => return Ok(()),
    };
    let path = match verbatim_template_sidecar(format, descriptor_path, edt_file, designer_file)
    {
        Some(p) => p,
        None => return Ok(()), // cf: container — no file-per-object sidecar to attach.
    };
    // The body is OPTIONAL: absent ⇒ no-op. s4_common has 10 property-stub CommonTemplates
    // (ОбщМакет_ТипМакета_* — testing `templateType` in isolation) WITHOUT a body file; the
    // earlier §1.0-STRICT "TextDocument MUST carry Template.txt" broke whole-config READ of s4.
    // Attach the body IFF the sidecar exists (reflects the on-disk state); downstream honesty stays
    // at ASSEMBLE (a TextDocument template with an attached body emits its <uuid>.0; a bodyless one
    // that reaches a body-requiring assemble path surfaces BodyNotEmitted loudly, never a silent
    // wrong body).
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    // Canonical body = BOM-stripped bytes (EDT has none, Designer has one → equal after strip).
    let body = bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec();
    // §1.0: never silently overwrite an already-attached body (the descriptor read must not
    // populate `templates` — only this pass does).
    if !obj.templates.is_empty() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries templates before the sidecar attach (unexpected — \
                     the descriptor projection must not populate them)"
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
    Ok(())
}

/// Child-level half of [`attach_template_body`]: attach each TextDocument `*.TemplateRef`
/// child's `Template.txt` body VERBATIM (BOM-stripped) in BOTH dialects — the body's EOL is
/// DATA and the platform preserves it exactly (module docs: `DumpConfigToFiles` oracle 17/17,
/// SSL 31/31 + ERP 854/854 sidecar pairs byte-identical). The body is OPTIONAL (a
/// descriptor-only stub attaches nothing — reflects the on-disk state, §1.0).
fn attach_child_text_bodies(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // container — no file-per-object sidecars.
    }
    for child in obj.children.iter_mut() {
        if !is_template_ref_child(child) {
            continue;
        }
        let ttype = template_type_of(child);
        let (edt_file, designer_file) = match verbatim_sidecar_files(ttype) {
            Some(files) => files,
            None => continue,
        };
        let path = match child_sidecar_path(
            format,
            descriptor_path,
            &child.name,
            edt_file,
            designer_file,
        ) {
            Some(p) => p,
            None => continue, // no parent/stem (defensive) — nothing to attach.
        };
        if !path.is_file() {
            continue; // OPTIONAL body — a bodyless stub round-trips descriptor-only.
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let stripped = bytes.strip_prefix(BOM).unwrap_or(&bytes);
        // Canonical body = the sidecar bytes VERBATIM after the BOM strip, in BOTH dialects and
        // for BOTH verbatim types: the platform stores/dumps a TextDocument body's newlines
        // EXACTLY (module docs — DumpConfigToFiles oracle 17/17 LF, corpus 885/885 pairs
        // byte-identical), and a DCAT body is platform-generated XML identical across dialects
        // modulo BOM (ERP object-level witness 5/5). Same rule as the object level.
        let body = stripped.to_vec();
        // §1.0: only this pass populates a TextDocument child's templates.
        if !child.templates.is_empty() {
            return Err(ConvertError::Read {
                kind: child.kind.as_str().to_string(),
                object: child.name.clone(),
                reason: "child already carries templates before the text sidecar attach \
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

/// Write-side mirror of [`attach_template_body`]: emit each attached TextDocument body beside
/// the descriptor just written, in the TARGET dialect. Covers the CommonTemplate OBJECT
/// (canonical body verbatim: Designer `Ext/Template.txt` = BOM + body, EDT `Template.txt` =
/// CRLF-normalized body) and every TextDocument `*.TemplateRef` CHILD (Designer = BOM +
/// canonical verbatim; EDT = CRLF-normalized — module docs). No-op for bodyless objects/
/// children and cf. §1.0: typed [`ConvertError`] on I/O.
pub fn write_template_bodies(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // container — bodies are assembled, not sidecar files.
    }
    // (a) CommonTemplate object-level body (verbatim types: TextDocument / DCAT).
    if TEMPLATE_SIDECAR_KINDS.contains(&kind) {
        let ttype = template_type_of(obj);
        if let Some((edt_file, designer_file)) = verbatim_sidecar_files(ttype) {
            if let Some(body) = obj.templates.iter().find_map(|t| t.body.as_ref()) {
                if let Some(path) =
                    verbatim_template_sidecar(format, descriptor_out, edt_file, designer_file)
                {
                    write_text_body(format, &path, body, ttype)?;
                }
            }
        }
    }
    // (b) verbatim-typed `*.TemplateRef` children.
    for child in &obj.children {
        if !is_template_ref_child(child) {
            continue;
        }
        let ttype = template_type_of(child);
        let (edt_file, designer_file) = match verbatim_sidecar_files(ttype) {
            Some(files) => files,
            None => continue,
        };
        let body = match child.templates.first().and_then(|t| t.body.as_ref()) {
            Some(b) => b,
            None => continue, // bodyless stub — mirrors the read's optional attach.
        };
        if let Some(path) =
            child_sidecar_path(format, descriptor_out, &child.name, edt_file, designer_file)
        {
            write_text_body(format, &path, body, ttype)?;
        }
    }
    Ok(())
}

/// Emit ONE verbatim-typed body at `path` in the target dialect: Designer = BOM + canonical
/// verbatim; EDT = BOM-less — CRLF-normalized for TextDocument, VERBATIM for DCAT (ERP witness
/// 5/5: EDT `Template.dcsat` == canon byte-exact).
///
/// The TextDocument CRLF normalization is EDT's OWN one-way export convention (`1cedtcli` forces
/// CRLF: s15 17/17 LF-in → CRLF-out; SSL 31/31 + ERP 854/854 already CRLF), so keeping it is
/// what makes `designer→edt` byte-equal to EDT's projection. It is deliberately NOT mirrored on
/// the READ side (module docs): the EOL is data, so `edt→*` re-emits it verbatim and an
/// LF-bodied canon simply cannot survive a trip through the EDT dialect — EDT's loss, not ours.
fn write_text_body(
    format: Format,
    path: &Path,
    body: &[u8],
    ttype: &str,
) -> Result<(), ConvertError> {
    let bytes = match format {
        Format::Designer => {
            let mut b = Vec::with_capacity(BOM.len() + body.len());
            b.extend_from_slice(BOM);
            b.extend_from_slice(body);
            b
        }
        Format::Edt if ttype == TEMPLATE_TYPE_TEXT_DOCUMENT => normalize_newlines(body, true),
        Format::Edt => body.to_vec(),
        Format::Cf => return Ok(()),
    };
    crate::form_write::write_file(path, &bytes)
}

/// Verbatim-typed sidecar path beside the descriptor, per format layout (see module docs):
/// EDT `<obj-dir>/<edt_file>`, Designer `<dir>/<Name>/Ext/<designer_file>`. `None` for formats
/// without a file-per-object sidecar (cf is a container).
fn verbatim_template_sidecar(
    format: Format,
    descriptor_path: &Path,
    edt_file: &str,
    designer_file: &str,
) -> Option<PathBuf> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/<edt_file>`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(obj_dir.join(edt_file))
        }
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/<designer_file>`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join(designer_file))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::Token;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn text_template(name: &str) -> MetadataObject {
        let mut o = MetadataObject::new(ObjectKind::new("CommonTemplate"), name, Uuid([1; 16]));
        o.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("TextDocument")),
        ));
        o
    }

    #[test]
    fn edt_sidecar_path_is_sibling() {
        let p = Path::new("/root/CommonTemplates/Макет/Макет.mdo");
        let s = verbatim_template_sidecar(Format::Edt, p, "Template.txt", "Template.txt").unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Template.txt")),
            "got {s:?}"
        );
        // DCAT: EDT sibling is `Template.dcsat`.
        let s =
            verbatim_template_sidecar(Format::Edt, p, "Template.dcsat", "Template.xml").unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Template.dcsat")),
            "got {s:?}"
        );
    }

    #[test]
    fn designer_sidecar_path_is_ext_subdir() {
        let p = Path::new("/root/CommonTemplates/Макет.xml");
        let s =
            verbatim_template_sidecar(Format::Designer, p, "Template.txt", "Template.txt").unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Ext/Template.txt")),
            "got {s:?}"
        );
        // DCAT: Designer file is `Ext/Template.xml` (disambiguated by templateType).
        let s = verbatim_template_sidecar(Format::Designer, p, "Template.dcsat", "Template.xml")
            .unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Ext/Template.xml")),
            "got {s:?}"
        );
    }

    /// DCAT object-level: attach VERBATIM from both dialects (EDT `Template.dcsat` bare,
    /// Designer `Ext/Template.xml` + BOM) → ONE canon; write back re-applies the per-format
    /// BOM convention WITHOUT newline transcoding (EDT verbatim — unlike TextDocument).
    #[test]
    fn dcat_object_attach_and_write_both_dialects() {
        let base = std::env::temp_dir().join(format!(
            "morph1c-dcat-obj-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let canon = b"<?xml version=\"1.0\"?>\r\n<AppearanceTemplate>\n</AppearanceTemplate>";

        let mut dcat = MetadataObject::new(
            ObjectKind::new("CommonTemplate"),
            "Оформление",
            Uuid([3; 16]),
        );
        dcat.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("DataCompositionAppearanceTemplate")),
        ));

        // EDT: bare sibling Template.dcsat.
        let edt_dir = base.join("edt/CommonTemplates/Оформление");
        std::fs::create_dir_all(&edt_dir).unwrap();
        crate::fsio::write(edt_dir.join("Template.dcsat"), canon).unwrap();
        let mut edt_obj = dcat.clone();
        attach_template_body(
            Format::Edt,
            "CommonTemplate",
            &edt_dir.join("Оформление.mdo"),
            &mut edt_obj,
        )
        .unwrap();
        assert_eq!(
            edt_obj.templates[0].body.as_deref(),
            Some(&canon[..]),
            "EDT DCAT body attaches VERBATIM (mixed EOL preserved — no CRLF transcoding)"
        );

        // Designer: Ext/Template.xml with BOM → same canon.
        let dz_ext = base.join("dz/CommonTemplates/Оформление/Ext");
        std::fs::create_dir_all(&dz_ext).unwrap();
        let mut with_bom = Vec::from(BOM);
        with_bom.extend_from_slice(canon);
        crate::fsio::write(dz_ext.join("Template.xml"), &with_bom).unwrap();
        let mut dz_obj = dcat.clone();
        attach_template_body(
            Format::Designer,
            "CommonTemplate",
            &base.join("dz/CommonTemplates/Оформление.xml"),
            &mut dz_obj,
        )
        .unwrap();
        assert_eq!(dz_obj.templates, edt_obj.templates, "one canon (§1.6)");

        // Write both dialects back byte-exact.
        let out_edt = base.join("out-edt/CommonTemplates/Оформление");
        std::fs::create_dir_all(&out_edt).unwrap();
        write_template_bodies(
            Format::Edt,
            "CommonTemplate",
            &out_edt.join("Оформление.mdo"),
            &dz_obj,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(out_edt.join("Template.dcsat")).unwrap(),
            canon.to_vec(),
            "EDT DCAT write = canon verbatim, BOM-less"
        );
        let out_dz = base.join("out-dz/CommonTemplates");
        std::fs::create_dir_all(&out_dz).unwrap();
        write_template_bodies(
            Format::Designer,
            "CommonTemplate",
            &out_dz.join("Оформление.xml"),
            &edt_obj,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(out_dz.join("Оформление/Ext/Template.xml")).unwrap(),
            with_bom,
            "Designer DCAT write = BOM + canon verbatim"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn non_template_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_template_body(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.templates.is_empty());
    }

    #[test]
    fn non_text_template_type_is_noop() {
        // A SpreadsheetDocument (default templateType) attaches NOTHING (its cf body is
        // structural, not a sidecar copy) — even though its sidecar path would not exist here.
        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonTemplate"), "Макет", Uuid([2; 16]));
        // no F_TEMPLATE_TYPE → default SpreadsheetDocument.
        attach_template_body(
            Format::Edt,
            "CommonTemplate",
            Path::new("/nope/CommonTemplates/Макет/Макет.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(
            obj.templates.is_empty(),
            "structural template type attaches no body"
        );
    }

    #[test]
    fn text_template_without_sidecar_is_noop() {
        // The body is OPTIONAL: a bodyless property-stub TextDocument template (e.g. s4_common's
        // `ОбщМакет_ТипМакета_ТекстовыйДокумент`) attaches NO body and does NOT error.
        let mut obj = text_template("Макет");
        attach_template_body(
            Format::Edt,
            "CommonTemplate",
            Path::new("/nonexistent-root/CommonTemplates/Макет/Макет.mdo"),
            &mut obj,
        )
        .expect("bodyless TextDocument template must be a no-op, not an error");
        assert!(
            obj.templates.is_empty(),
            "no body attached when the sidecar is absent"
        );
    }

    #[test]
    fn attaches_edt_text_sidecar_bom_stripped() {
        // Self-contained: write an EDT Template.txt (no BOM), attach it, assert canonical body.
        let base = std::env::temp_dir().join(format!(
            "morph1c-template-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let obj_dir = base.join("CommonTemplates").join("Макет");
        std::fs::create_dir_all(&obj_dir).unwrap();
        crate::fsio::write(obj_dir.join("Template.txt"), b"1;Row\r\n2;Row\r\n").unwrap();
        let descriptor = obj_dir.join("Макет.mdo");

        let mut obj = text_template("Макет");
        attach_template_body(Format::Edt, "CommonTemplate", &descriptor, &mut obj).unwrap();
        assert_eq!(obj.templates.len(), 1, "one template body attached");
        assert_eq!(
            obj.templates[0].body.as_deref(),
            Some(b"1;Row\r\n2;Row\r\n".as_slice())
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn normalize_newlines_both_directions() {
        assert_eq!(normalize_newlines(b"a\r\nb\nc", true), b"a\r\nb\r\nc");
        assert_eq!(normalize_newlines(b"a\r\nb\nc", false), b"a\nb\nc");
        // A lone CR (no following LF) is data, not a line ending — left as-is.
        assert_eq!(normalize_newlines(b"lone\rcr", true), b"lone\rcr");
        assert_eq!(normalize_newlines(b"", true), b"");
    }

    fn text_template_child(name: &str) -> MetadataObject {
        let mut c = MetadataObject::new(
            ObjectKind::new("DataProcessor.TemplateRef"),
            name,
            Uuid([7; 16]),
        );
        c.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("TextDocument")),
        ));
        c
    }

    #[test]
    fn child_sidecar_paths_per_format() {
        let edt_host = Path::new("/r/DataProcessors/Обр/Обр.mdo");
        let s = child_sidecar_path(
            Format::Edt,
            edt_host,
            "Макет",
            "Template.txt",
            "Template.txt",
        )
        .unwrap();
        assert!(
            s.ends_with(Path::new("DataProcessors/Обр/Templates/Макет/Template.txt")),
            "got {s:?}"
        );
        let dz_host = Path::new("/r/DataProcessors/Обр.xml");
        let s = child_sidecar_path(
            Format::Designer,
            dz_host,
            "Макет",
            "Template.txt",
            "Template.txt",
        )
        .unwrap();
        assert!(
            s.ends_with(Path::new(
                "DataProcessors/Обр/Templates/Макет/Ext/Template.txt"
            )),
            "got {s:?}"
        );
        assert!(child_sidecar_path(Format::Cf, dz_host, "Макет", "a", "b").is_none());
    }

    #[test]
    fn attaches_edt_child_text_body_verbatim() {
        // EDT child body is CRLF on disk; the attach keeps it VERBATIM — the EOL is DATA
        // (module docs: platform DumpConfigToFiles oracle 17/17, corpus 885/885 pairs).
        let base = std::env::temp_dir().join(format!(
            "morph1c-tmpl-child-edt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let tdir = base
            .join("DataProcessors")
            .join("Обр")
            .join("Templates")
            .join("Макет");
        std::fs::create_dir_all(&tdir).unwrap();
        crate::fsio::write(tdir.join("Template.txt"), b"first\r\nsecond\r\n").unwrap();
        let host = base.join("DataProcessors").join("Обр").join("Обр.mdo");

        let mut obj = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj.children.push(text_template_child("Макет"));
        attach_template_body(Format::Edt, "DataProcessor", &host, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].templates[0].body.as_deref(),
            Some(b"first\r\nsecond\r\n".as_slice()),
            "EDT child text body attaches VERBATIM (EOL is data)"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attaches_designer_child_text_body_verbatim() {
        // Designer child body = cf-verbatim ground truth: BOM stripped, newlines untouched.
        let base = std::env::temp_dir().join(format!(
            "morph1c-tmpl-child-dz-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ext = base
            .join("DataProcessors")
            .join("Обр")
            .join("Templates")
            .join("Макет")
            .join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        let mut with_bom = Vec::from(BOM);
        with_bom.extend_from_slice(b"line\n"); // LF stays LF (verbatim).
        crate::fsio::write(ext.join("Template.txt"), &with_bom).unwrap();
        let host = base.join("DataProcessors").join("Обр.xml");

        let mut obj = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj.children.push(text_template_child("Макет"));
        attach_template_body(Format::Designer, "DataProcessor", &host, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].templates[0].body.as_deref(),
            Some(b"line\n".as_slice())
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bodyless_or_nontext_children_attach_nothing() {
        // A descriptor-only TextDocument stub (no sidecar) and a bodyless structural type
        // (GraphicalSchema) both round-trip descriptor-only: nothing attached, no error (§1.0).
        let mut obj = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj.children.push(text_template_child("МакетТекст"));
        let mut graph = MetadataObject::new(
            ObjectKind::new("DataProcessor.TemplateRef"),
            "МакетГраф",
            Uuid([8; 16]),
        );
        graph.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("GraphicalSchema")),
        ));
        obj.children.push(graph);
        let host = Path::new("/nonexistent-root/DataProcessors/Обр/Обр.mdo");
        attach_template_body(Format::Edt, "DataProcessor", host, &mut obj).unwrap();
        assert!(obj.children.iter().all(|c| c.templates.is_empty()));
    }

    #[test]
    fn write_child_text_bodies_reapply_dialect_conventions() {
        // Canonical child body (LF) → EDT re-normalizes to CRLF (BOM-less), Designer emits
        // BOM + canonical verbatim.
        let base = std::env::temp_dir().join(format!(
            "morph1c-tmpl-child-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let mut child = text_template_child("Макет");
        child.templates.push(Template {
            pages: Vec::new(),
            resources: Vec::new(),
            name: child.name.clone(),
            properties: Vec::new(),
            body: Some(b"first\nsecond\n".to_vec()),
        });
        let mut obj = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj.children.push(child);

        std::fs::create_dir_all(base.join("edt").join("DataProcessors").join("Обр")).unwrap();
        let edt_host = base
            .join("edt")
            .join("DataProcessors")
            .join("Обр")
            .join("Обр.mdo");
        write_template_bodies(Format::Edt, "DataProcessor", &edt_host, &obj).unwrap();
        let edt = std::fs::read(base.join("edt/DataProcessors/Обр/Templates/Макет/Template.txt"))
            .unwrap();
        assert_eq!(edt, b"first\r\nsecond\r\n".to_vec(), "EDT = CRLF, BOM-less");

        std::fs::create_dir_all(base.join("dz").join("DataProcessors")).unwrap();
        let dz_host = base.join("dz").join("DataProcessors").join("Обр.xml");
        write_template_bodies(Format::Designer, "DataProcessor", &dz_host, &obj).unwrap();
        let dz = std::fs::read(base.join("dz/DataProcessors/Обр/Templates/Макет/Ext/Template.txt"))
            .unwrap();
        let mut expected = Vec::from(BOM);
        expected.extend_from_slice(b"first\nsecond\n");
        assert_eq!(dz, expected, "Designer = BOM + canonical verbatim");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_object_level_common_template_text_body() {
        // The CommonTemplate OBJECT body (canonical = verbatim as read; SSL witness CRLF) is
        // re-emitted: Designer = BOM + verbatim, EDT = CRLF-normalized (identical for a CRLF
        // canonical — the SSL pair is byte-identical across dialects).
        let base = std::env::temp_dir().join(format!(
            "morph1c-tmpl-obj-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let mut obj = text_template("Макет");
        obj.templates.push(Template {
            pages: Vec::new(),
            resources: Vec::new(),
            name: obj.name.clone(),
            properties: Vec::new(),
            body: Some(b"1;Row\r\n2;Row\r\n".to_vec()),
        });

        std::fs::create_dir_all(base.join("edt").join("CommonTemplates").join("Макет")).unwrap();
        let edt_desc = base
            .join("edt")
            .join("CommonTemplates")
            .join("Макет")
            .join("Макет.mdo");
        write_template_bodies(Format::Edt, "CommonTemplate", &edt_desc, &obj).unwrap();
        let edt = std::fs::read(base.join("edt/CommonTemplates/Макет/Template.txt")).unwrap();
        assert_eq!(edt, b"1;Row\r\n2;Row\r\n".to_vec());

        std::fs::create_dir_all(base.join("dz").join("CommonTemplates")).unwrap();
        let dz_desc = base.join("dz").join("CommonTemplates").join("Макет.xml");
        write_template_bodies(Format::Designer, "CommonTemplate", &dz_desc, &obj).unwrap();
        let dz = std::fs::read(base.join("dz/CommonTemplates/Макет/Ext/Template.txt")).unwrap();
        let mut expected = Vec::from(BOM);
        expected.extend_from_slice(b"1;Row\r\n2;Row\r\n");
        assert_eq!(
            dz, expected,
            "Designer = BOM + verbatim (CRLF preserved — SSL witness)"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attaches_designer_text_sidecar_strips_bom() {
        // Designer Ext/Template.txt carries a BOM; the canonical body strips it (== EDT body).
        let base = std::env::temp_dir().join(format!(
            "morph1c-template-read-des-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ext_dir = base.join("CommonTemplates").join("Макет").join("Ext");
        std::fs::create_dir_all(&ext_dir).unwrap();
        let mut with_bom = Vec::from(BOM);
        with_bom.extend_from_slice(b"1;Row\r\n2;Row\r\n");
        crate::fsio::write(ext_dir.join("Template.txt"), &with_bom).unwrap();
        let descriptor = base.join("CommonTemplates").join("Макет.xml");

        let mut obj = text_template("Макет");
        attach_template_body(Format::Designer, "CommonTemplate", &descriptor, &mut obj).unwrap();
        assert_eq!(
            obj.templates[0].body.as_deref(),
            Some(b"1;Row\r\n2;Row\r\n".as_slice()),
            "Designer body BOM-stripped == EDT canonical body (§1.6)"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
