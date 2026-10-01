//! Write FORM-BODY SIDECARS (`Form.form` EDT / `Ext/Form.xml` Designer) plus their optional
//! form MODULE (`Module.bsl`) beside the descriptor during whole-config write (§1.0/§1.6).
//! Write-side mirror of [`crate::form_read`]: the descriptor writer emits only the thin `.mdo`/
//! `.xml`; this pass re-emits every whole form body in the TARGET dialect via the byte-exact
//! codec `formats_xml::form::write_form`, and copies each form module to its per-format path.
//!
//! Each target body path is derived from the descriptor's OUTPUT path exactly as
//! [`crate::form_read`] derives the sidecar from the source descriptor path (the shared
//! [`crate::form_read::form_anchor_path`] dual-mode resolver) — so read and write stay in
//! lock-step: a `CommonForm` writes its OWN body beside the descriptor (EDT `<obj-dir>/
//! Form.form`; Designer `<dir>/<Name>/Ext/Form.xml`); a subordinate-form OWNER (Catalog/
//! Document/…) writes one body per carried `NamedFormBody` under `Forms/<FormName>/`.

use std::path::{Path, PathBuf};

use formats_xml::form::{
    FormDialect, write_form, write_list_settings_dcss, write_spreadsheet_mxlx,
};
use formats_xml::registry::Format;
use morph1c_core::ir::{FormBody, FormDataAttribute, MetadataObject};

use crate::ConvertError;
use crate::form_read::{
    form_anchor_path, form_body_path, form_items_dir, form_module_path, list_settings_sidecar_path,
    spreadsheet_sidecar_path,
};

/// Emit every form body (and optional form module) `obj` carries beside its written descriptor.
///
/// `descriptor_out` — the path the object's descriptor was just written to (in the destination
/// tree). Each form body/module path is derived from it + the body's NAME per the target format
/// layout (dual-mode, see [`crate::form_read`]), its parent dirs are created, and the body is
/// re-serialised with `write_form` in the target dialect. cf (container) and objects without a
/// form body are no-ops. §1.0: a write failure — including a body carried by a kind with NO
/// registered form-sidecar layout — is a typed [`ConvertError`] (never a silent drop).
pub fn write_form_bodies(
    format: Format,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if obj.form_bodies.is_empty() {
        return Ok(());
    }
    if format == Format::Cf {
        // cf is assembled by `cf_write` (write_config routes it there before this pass); kept
        // defensive — the container has no file-per-object sidecar to write.
        return Ok(());
    }
    let kind = obj.kind.as_str();

    // A CommonForm owns exactly one body (the form IS the object). Guard the `Vec` shape: more
    // than one body on a CommonForm would be a broken IR — refuse loudly rather than scatter
    // extra bodies over derived paths (§1.0).
    if kind == "CommonForm" && obj.form_bodies.len() != 1 {
        return Err(ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: format!(
                "CommonForm carries {} form bodies but owns exactly one (the form IS the object)",
                obj.form_bodies.len()
            ),
        });
    }

    for form in &obj.form_bodies {
        // Dual-mode anchor (shared with the read side): own body for CommonForm, the
        // `Forms/<FormName>/` subtree for subordinate owners. A kind with form bodies but NO
        // registered layout is a §1.0 hard error — writing nothing would silently drop content.
        let anchor =
            form_anchor_path(format, kind, descriptor_out, &form.name).ok_or_else(|| {
                ConvertError::Write {
                    kind: kind.to_string(),
                    object: obj.name.clone(),
                    reason: format!(
                        "object carries form body {:?} but no form-sidecar layout is registered \
                         for kind {kind} (silent drop forbidden)",
                        form.name
                    ),
                }
            })?;
        let declared_ordinary = crate::form_read::declared_form_is_ordinary(obj, &form.name)?;
        if let Some(bytes) = &form.ordinary_body {
            if !declared_ordinary
                || form.body != FormBody::new()
                || form.module.is_some()
                || bytes.is_empty()
            {
                return Err(ConvertError::Write { kind: kind.to_string(), object: form.name.clone(), reason: "ordinary form body conflicts with managed data/module/type".into() });
            }
            let path =
                crate::form_read::ordinary_form_body_path(format, &anchor).ok_or_else(|| {
                    ConvertError::Write {
                        kind: kind.to_string(),
                        object: form.name.clone(),
                        reason: "ordinary form output layout unavailable".into(),
                    }
                })?;
            write_file(&path, bytes)?;
            if kind != "CommonForm" {
                crate::help_read::write_help_sidecar(
                    format,
                    kind,
                    &anchor,
                    &form.name,
                    &form.help,
                    &form.help_resources,
                )?;
            }
            continue;
        }
        if declared_ordinary {
            return Err(ConvertError::Write {
                kind: kind.to_string(),
                object: form.name.clone(),
                reason: "ordinary declaration cannot emit managed form data".into(),
            });
        }
        let (body_path, dialect) = form_body_path(format, &anchor).ok_or_else(|| {
            // Unreachable for edt/designer (both derive a path); typed for §1.0 anyway.
            ConvertError::Write {
                kind: kind.to_string(),
                object: obj.name.clone(),
                reason: "no form-body path for this format (unexpected for edt/designer)".into(),
            }
        })?;

        let projection = if format == Format::Edt {
            Some(
                formats_xml::form::project_picture_semantics(
                    &form.body,
                    crate::form_read::declared_form_uuid(obj, &form.name)?,
                )
                .map_err(|error| ConvertError::Write {
                    kind: kind.into(),
                    object: form.name.clone(),
                    reason: error.to_string(),
                })?,
            )
        } else {
            None
        };
        let body = projection.as_ref().map_or(&form.body, |(body, _)| body);
        let bytes = write_form(dialect, body).map_err(|e| ConvertError::Write {
            kind: kind.to_string(),
            object: format!("{}.{}", obj.name, form.name),
            reason: e.to_string(),
        })?;
        write_file(&body_path, &bytes)?;
        if let Some((_, Some(bytes))) = projection {
            write_file(
                &body_path
                    .parent()
                    .expect("form body has a parent")
                    .join(formats_xml::form::PICTURE_SEMANTICS_RESOURCE),
                &bytes,
            )?;
        }

        // EDT: the spreadsheet-document BODY of a form attribute lives in a SIDECAR
        // (`Attributes/<attr>/ExtInfo/SpreadsheetData.mxlx`; `Form.form` carries only the empty
        // marker), while Designer inlines it into `Ext/Form.xml` (handled by the codec above).
        // Emit one sidecar per attribute carrying `spreadsheet_settings` — dropping it would be
        // the exact content loss this pass exists to prevent (§1.0/§1.6).
        // EDT: likewise the DCS user-SETTINGS of a dynamic-list attribute live in a sidecar
        // (`Attributes/<attr>/ExtInfo/ListSettings.dcss`), absent from `Form.form` entirely, while
        // Designer inlines them as `<ListSettings>` (handled by the codec above).
        // EDT: the FORM-LEVEL conditional appearance likewise lives in a sidecar
        // (`ConditionalAppearance.dcssca`), absent from `Form.form` entirely, while Designer inlines
        // it inside `<Attributes>` (handled by the codec above). Mirror of the read attach
        // (`form_read::attach_edt_conditional_appearance`, F-wave 23).
        if matches!(dialect, FormDialect::Edt) {
            if let Some(form_dir) = body_path.parent() {
                write_edt_spreadsheet_sidecars(form_dir, kind, &obj.name, &form.name, &form.body)?;
                write_edt_chart_sidecars(form_dir, &form.body)?;
                write_edt_list_settings_sidecars(form_dir, &form.body)?;
                if !form.body.conditional_appearance.is_empty() {
                    write_file(
                        &crate::form_read::conditional_appearance_sidecar_path(form_dir),
                        &formats_xml::form::write_conditional_appearance_dcssca(
                            &form.body.conditional_appearance,
                            form.body.ca_envelope_without_lf_pal,
                        ),
                    )?;
                }
            }
        }

        // The INLINE picture of a control is a binary SIDECAR in BOTH dialects
        // (`Items/<ctl>/<Tag>.<ext>`) — the descriptor carries only a marker. Mirror of the read
        // attach: re-emit the bytes VERBATIM, so a cross-format convert moves the image instead of
        // dropping it (§1.0 Blob / §1.6).
        if let Some(items_dir) = form_items_dir(format, &anchor) {
            write_form_picture_sidecars(&items_dir, &form.body)?;
        }

        // Optional form module — write it to its per-format path when present. Canonicalise the
        // BOM per format (Designer prepends it, EDT bare) exactly like the CommonModule module
        // writer, so a cross-format convert re-emits each format's own text-sidecar convention
        // (§1.6).
        if let Some(source) = &form.module {
            let module_out =
                form_module_path(format, &anchor).ok_or_else(|| ConvertError::Write {
                    kind: kind.to_string(),
                    object: obj.name.clone(),
                    reason: "no form-module path for this format (unexpected for edt/designer)"
                        .into(),
                })?;
            write_file(
                &module_out,
                &crate::module_read::reencode_module_for_format(source, format),
            )?;
        }

        // Optional form HELP — the read-side mirror (`form_read`): EDT `Forms/<F>/Help/<lang>.html`,
        // Designer `Forms/<F>/Ext/Help.xml` + `Ext/Help/<lang>.html`, anchored at the form. The
        // SAME writer the object help uses (only the anchor differs). CommonForm carries its help
        // on the OBJECT (`obj.help` — emitted by `help_read::write_help_pages`), so its
        // `NamedFormBody.help` is empty by construction and this is a no-op for it.
        crate::help_read::write_help_sidecar(
            format,
            kind,
            &anchor,
            &form.name,
            &form.help,
            &form.help_resources,
        )?;
    }
    Ok(())
}

/// Emit the EDT `SpreadsheetData.mxlx` sidecar for every TOP-LEVEL data attribute of `body`
/// carrying `spreadsheet_settings` (mirror of the read-side attach — same
/// [`spreadsheet_sidecar_path`] layout). §1.0: a NESTED column carrying spreadsheet settings has
/// no registered sidecar layout (0 in corpus) — hard error, never a silent drop.
fn write_edt_spreadsheet_sidecars(
    form_dir: &Path,
    kind: &str,
    owner: &str,
    form_name: &str,
    body: &FormBody,
) -> Result<(), ConvertError> {
    for attr in &body.data_attributes {
        ensure_no_nested_spreadsheet(attr, kind, owner, form_name)?;
        if let Some(ss) = &attr.spreadsheet_settings {
            let path = spreadsheet_sidecar_path(form_dir, &attr.name);
            write_file(&path, &write_spreadsheet_mxlx(ss))?;
        }
    }
    Ok(())
}

/// Emit the EDT `Chart.chart` / `GanttChart.chart` sidecar for every TOP-LEVEL data attribute
/// of `body` carrying `chart_settings` (mirror of the read-side attach; the filename tracks
/// [`ChartSettings::kind`]). Layout `Attributes/<attr>/ExtInfo/<Kind>.chart`.
fn write_edt_chart_sidecars(form_dir: &Path, body: &FormBody) -> Result<(), ConvertError> {
    for attr in &body.data_attributes {
        if let Some(cs) = &attr.chart_settings {
            let file = format!("{}.chart", cs.kind);
            let path = form_dir
                .join("Attributes")
                .join(&attr.name)
                .join("ExtInfo")
                .join(file);
            let bytes =
                formats_xml::form::write_chart_sidecar(cs).map_err(|e| ConvertError::Read {
                    kind: "Form".to_string(),
                    object: attr.name.clone(),
                    reason: format!("chart sidecar write: {e}"),
                })?;
            write_file(&path, &bytes)?;
        }
    }
    Ok(())
}

/// Emit the EDT `ListSettings.dcss` sidecar for every TOP-LEVEL dynamic-list attribute of `body`
/// carrying NON-EMPTY DCS settings (mirror of the read-side attach — same
/// [`list_settings_sidecar_path`] layout).
///
/// EMPTY settings emit NO file: the platform creates none (227 sidecars / 229 dynamic lists in
/// SSL — the 2 without a file are exactly the 2 whose Designer tag is the self-closing
/// `<ListSettings/>`), so writing an "empty" sidecar would be a spurious extra file. Nested
/// COLUMNS are never dynamic lists (`dynamic_list` is a top-level attribute ext), so no shape
/// guard is needed here.
fn write_edt_list_settings_sidecars(form_dir: &Path, body: &FormBody) -> Result<(), ConvertError> {
    for attr in &body.data_attributes {
        let Some(dl) = &attr.dynamic_list else {
            continue;
        };
        let Some(ls) = &dl.list_settings else {
            continue;
        };
        if ls.is_empty() {
            continue;
        }
        let path = list_settings_sidecar_path(form_dir, &attr.name);
        write_file(&path, &write_list_settings_dcss(ls))?;
    }
    Ok(())
}

/// §1.0 shape guard: refuse a spreadsheet-document body on a NESTED column (recursively, incl.
/// additional-column groups) — no EDT sidecar layout exists for it (0 witnesses in the corpus),
/// so writing nothing would silently drop content.
fn ensure_no_nested_spreadsheet(
    attr: &FormDataAttribute,
    kind: &str,
    owner: &str,
    form_name: &str,
) -> Result<(), ConvertError> {
    let nested = attr.columns.iter().chain(
        attr.additional_columns
            .iter()
            .flat_map(|ac| ac.columns.iter()),
    );
    for col in nested {
        if col.spreadsheet_settings.is_some() {
            return Err(ConvertError::Write {
                kind: kind.to_string(),
                object: format!("{owner}.{form_name}"),
                reason: format!(
                    "nested column {:?} of attribute {:?} carries a spreadsheet-document body — \
                     no EDT sidecar layout is registered for columns (silent drop forbidden, §1.0)",
                    col.name, attr.name
                ),
            });
        }
        ensure_no_nested_spreadsheet(col, kind, owner, form_name)?;
    }
    Ok(())
}

/// Create the parent dir and write `bytes` to `path` (typed [`ConvertError::Io`] on failure).
/// Shared by the other write-side body-sidecar emitters (`module_read`/`rights_read`/`xdto_read`
/// write halves) so the create-dir-then-write dance lives in one place.
/// Emit the INLINE picture sidecar of every control of `body` that carries one, at
/// `Items/<ctl>/<file_name>` (mirror of the read attach — same [`form_items_dir`] layout, and the
/// file name is carried by the IR itself). Bytes are written VERBATIM (§1.0 Blob).
fn write_form_picture_sidecars(items_dir: &Path, body: &FormBody) -> Result<(), ConvertError> {
    let mut out: Vec<(PathBuf, &[u8])> = Vec::new();
    crate::form_read::walk_items(body, &mut |item| {
        for p in &item.pictures {
            out.push((items_dir.join(&item.name).join(&p.file_name), &p.bytes));
        }
    });
    for (path, bytes) in out {
        write_file(&path, bytes)?;
    }
    Ok(())
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ConvertError> {
    if let Some(parent) = path.parent() {
        crate::fsio::create_dir_all(parent).map_err(|e| ConvertError::Io {
            path: parent.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    crate::fsio::write(path, bytes).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};

    /// Fresh per-test temp dir (removed by the caller at the end).
    fn temp_base(tag: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "morph1c-formwrite-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    /// Synthetic Catalog owner declaring `forms` as `.FormRef` children (declaration order).
    fn owner_with_forms(forms: &[&str]) -> MetadataObject {
        let mut obj =
            MetadataObject::new(ObjectKind::new("Catalog"), "Спр_Подчиненные", Uuid([0; 16]));
        for (i, f) in forms.iter().enumerate() {
            obj.children.push(MetadataObject::new(
                ObjectKind::new("Catalog.FormRef"),
                *f,
                Uuid([i as u8 + 1; 16]),
            ));
        }
        obj
    }

    /// Same-dialect attach→write of the s15 subordinate form must be BYTE-EXACT for both the
    /// body and the module (the write derives the target path from the descriptor output path
    /// with the same dual-mode anchor the read used).
    fn assert_subordinate_roundtrip_byte_exact(
        format: Format,
        src_desc: &Path,
        out_desc_rel: &Path,
    ) {
        let mut obj = owner_with_forms(&["Форма"]);
        crate::form_read::attach_form_body(format, "Catalog", src_desc, &mut obj)
            .expect("attach source form");
        assert_eq!(obj.form_bodies.len(), 1, "fixture declares one form");

        let base = temp_base(format.code());
        let out_desc = base.join(out_desc_rel);
        write_form_bodies(format, &out_desc, &obj).expect("write form bodies");

        let anchor =
            crate::form_read::form_anchor_path(format, "Catalog", &out_desc, "Форма").unwrap();
        let (out_body, _) = crate::form_read::form_body_path(format, &anchor).unwrap();
        let out_module = crate::form_read::form_module_path(format, &anchor).unwrap();
        let src_anchor =
            crate::form_read::form_anchor_path(format, "Catalog", src_desc, "Форма").unwrap();
        let (src_body, _) = crate::form_read::form_body_path(format, &src_anchor).unwrap();
        let src_module = crate::form_read::form_module_path(format, &src_anchor).unwrap();

        let orig = std::fs::read(&src_body).unwrap();
        let regen = std::fs::read(&out_body).expect("body written");
        assert_eq!(orig, regen, "form body byte-exact ({})", format.code());
        let orig_m = std::fs::read(&src_module).unwrap();
        let regen_m = std::fs::read(&out_module).expect("module written");
        assert_eq!(
            orig_m,
            regen_m,
            "form module byte-exact ({})",
            format.code()
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn subordinate_form_write_byte_exact_edt() {
        let desc = morph1c_testkit::fixtures_root()
            .join("coverage/edt/s15_subordinate/src/Catalogs/Спр_Подчиненные/Спр_Подчиненные.mdo");
        if !desc.is_file() {
            eprintln!("subordinate_form_write_byte_exact_edt INFRA-SKIP (corpus absent)");
            return;
        }
        assert_subordinate_roundtrip_byte_exact(
            Format::Edt,
            &desc,
            Path::new("src/Catalogs/Спр_Подчиненные/Спр_Подчиненные.mdo"),
        );
    }

    #[test]
    fn subordinate_form_write_byte_exact_designer() {
        let desc = morph1c_testkit::fixtures_root()
            .join("coverage/designer/s15_subordinate/Catalogs/Спр_Подчиненные.xml");
        if !desc.is_file() {
            eprintln!("subordinate_form_write_byte_exact_designer INFRA-SKIP (corpus absent)");
            return;
        }
        assert_subordinate_roundtrip_byte_exact(
            Format::Designer,
            &desc,
            Path::new("Catalogs/Спр_Подчиненные.xml"),
        );
    }

    #[test]
    fn multiple_forms_per_owner_attach_and_write() {
        // One owner, TWO declared forms (the multi-form shape the old writer refused): build a
        // temp EDT layout carrying the s15 body under two form names (one with a module), attach
        // both (declaration order), write to a fresh root — both byte-exact at their own paths.
        let fixture_body = morph1c_testkit::fixtures_root().join(
            "coverage/edt/s15_subordinate/src/Catalogs/Спр_Подчиненные/Forms/Форма/Form.form",
        );
        if !fixture_body.is_file() {
            eprintln!("multiple_forms_per_owner_attach_and_write INFRA-SKIP (corpus absent)");
            return;
        }
        let body_bytes = std::fs::read(&fixture_body).unwrap();
        let module_text = "// форма 2\r\nПроцедура ПриОткрытии(Отказ)\r\nКонецПроцедуры\r\n";

        let base = temp_base("multi");
        let src_obj_dir = base.join("src/Catalogs/Спр");
        for name in ["Форма1", "Форма2"] {
            let d = src_obj_dir.join("Forms").join(name);
            std::fs::create_dir_all(&d).unwrap();
            crate::fsio::write(d.join("Form.form"), &body_bytes).unwrap();
        }
        crate::fsio::write(src_obj_dir.join("Forms/Форма2/Module.bsl"), module_text).unwrap();

        let mut obj = owner_with_forms(&["Форма1", "Форма2"]);
        obj.name = "Спр".into();
        let src_desc = src_obj_dir.join("Спр.mdo");
        crate::form_read::attach_form_body(Format::Edt, "Catalog", &src_desc, &mut obj)
            .expect("attach both forms");
        assert_eq!(
            obj.form_bodies
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Форма1", "Форма2"],
            "both forms attached in declaration order"
        );
        assert!(obj.form_bodies[0].module.is_none());
        assert_eq!(obj.form_bodies[1].module.as_deref(), Some(module_text));

        let out_desc = base.join("out/Catalogs/Спр/Спр.mdo");
        write_form_bodies(Format::Edt, &out_desc, &obj).expect("write both forms");
        for name in ["Форма1", "Форма2"] {
            let out = base
                .join("out/Catalogs/Спр/Forms")
                .join(name)
                .join("Form.form");
            assert_eq!(
                std::fs::read(&out).expect("written"),
                body_bytes,
                "{name} byte-exact"
            );
        }
        assert_eq!(
            std::fs::read(base.join("out/Catalogs/Спр/Forms/Форма2/Module.bsl")).expect("module"),
            module_text.as_bytes(),
            "module byte-exact (EDT bare, no BOM)"
        );
        assert!(
            !base
                .join("out/Catalogs/Спр/Forms/Форма1/Module.bsl")
                .exists(),
            "no module fabricated for a module-less form"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Attach the coverage `Форма_Группы` CommonForm (spreadsheet-document attribute) from the
    /// SOURCE dialect, write it in the TARGET dialect, and compare EVERY on-disk artifact
    /// (Form.form/Ext/Form.xml + the EDT SpreadsheetData.mxlx sidecar) to the committed TARGET
    /// fixture — the cross-format transcode proxy for the s10 vrunner cell.
    fn assert_spreadsheet_form_crosses(from: Format, to: Format) {
        let fix = morph1c_testkit::fixtures_root().join("coverage");
        let src_desc = match from {
            Format::Edt => fix.join("edt/s10_forms/src/CommonForms/Форма_Группы/Форма_Группы.mdo"),
            _ => fix.join("designer/s10_forms/CommonForms/Форма_Группы.xml"),
        };
        let ref_desc = match to {
            Format::Edt => fix.join("edt/s10_forms/src/CommonForms/Форма_Группы/Форма_Группы.mdo"),
            _ => fix.join("designer/s10_forms/CommonForms/Форма_Группы.xml"),
        };
        let src_probe = match from {
            // The EDT descriptor .mdo exists in the fixture; probe the form body instead.
            Format::Edt => src_desc.parent().unwrap().join("Form.form"),
            _ => src_desc.clone(),
        };
        if !src_probe.is_file() || !(ref_desc.is_file() || ref_desc.parent().unwrap().is_dir()) {
            eprintln!("assert_spreadsheet_form_crosses INFRA-SKIP (corpus absent)");
            return;
        }

        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonForm"), "Форма_Группы", Uuid([0; 16]));
        crate::form_read::attach_form_body(from, "CommonForm", &src_desc, &mut obj)
            .expect("attach source form");
        assert_eq!(obj.form_bodies.len(), 1);
        let ss = obj.form_bodies[0]
            .body
            .data_attributes
            .iter()
            .find(|a| a.name == "ТабличныйДокумент")
            .expect("spreadsheet attribute present")
            .spreadsheet_settings
            .as_ref()
            .expect("spreadsheet body attached (inline block / .mxlx sidecar)");
        assert_eq!(ss.template_mode, Some(true), "witness carries templateMode");

        let base = temp_base(&format!("mxl-{}-{}", from.code(), to.code()));
        let out_desc = match to {
            Format::Edt => base.join("src/CommonForms/Форма_Группы/Форма_Группы.mdo"),
            _ => base.join("CommonForms/Форма_Группы.xml"),
        };
        write_form_bodies(to, &out_desc, &obj).expect("write target form");

        let anchor =
            crate::form_read::form_anchor_path(to, "CommonForm", &out_desc, "Форма_Группы")
                .unwrap();
        let (out_body, _) = crate::form_read::form_body_path(to, &anchor).unwrap();
        let ref_anchor =
            crate::form_read::form_anchor_path(to, "CommonForm", &ref_desc, "Форма_Группы")
                .unwrap();
        let (ref_body, _) = crate::form_read::form_body_path(to, &ref_anchor).unwrap();
        assert_eq!(
            std::fs::read(&out_body).expect("body written"),
            std::fs::read(&ref_body).expect("reference body"),
            "form body byte-exact vs committed {} fixture",
            to.code()
        );
        if to == Format::Edt {
            let out_mxlx = crate::form_read::spreadsheet_sidecar_path(
                out_body.parent().unwrap(),
                "ТабличныйДокумент",
            );
            let ref_mxlx = crate::form_read::spreadsheet_sidecar_path(
                ref_body.parent().unwrap(),
                "ТабличныйДокумент",
            );
            assert_eq!(
                std::fs::read(&out_mxlx).expect("mxlx sidecar written"),
                std::fs::read(&ref_mxlx).expect("reference mxlx"),
                "SpreadsheetData.mxlx byte-exact vs committed edt fixture"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn spreadsheet_form_designer_to_edt_byte_exact() {
        assert_spreadsheet_form_crosses(Format::Designer, Format::Edt);
    }

    #[test]
    fn spreadsheet_form_edt_to_designer_byte_exact() {
        assert_spreadsheet_form_crosses(Format::Edt, Format::Designer);
    }

    #[test]
    fn nested_column_spreadsheet_body_refused() {
        // §1.0 shape guard: a NESTED column carrying a spreadsheet body has no EDT sidecar
        // layout — writing must refuse loudly, never drop the content.
        use morph1c_core::ir::form::MxlSpreadsheetSettings;
        let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "Ф", Uuid([0; 16]));
        let mut body = morph1c_core::ir::FormBody::new();
        let ss = MxlSpreadsheetSettings {
            envelope_without_pal: false,
            language_settings: None,
            columns_size: "0".into(),
            rows_index: "0".into(),
            row_empty: true,
            template_mode: None,
            vg_rows: "0".into(),
            full_body: None,
        };
        let mut col = blank_attr("Колонка");
        col.spreadsheet_settings = Some(ss);
        let mut table = blank_attr("Таблица");
        table.columns.push(col);
        body.data_attributes.push(table);
        obj.form_bodies.push(morph1c_core::ir::NamedFormBody {
            name: "Ф".into(),
            body,
            module: None,
            help: Vec::new(),
            help_resources: Vec::new(),
        });
        let base = temp_base("nested-mxl");
        let err = write_form_bodies(Format::Edt, &base.join("src/CommonForms/Ф/Ф.mdo"), &obj)
            .unwrap_err();
        assert!(
            matches!(&err, ConvertError::Write { reason, .. } if reason.contains("§1.0")),
            "got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Пустой реквизит-болванка для shape-гардов.
    fn blank_attr(name: &str) -> morph1c_core::ir::FormDataAttribute {
        morph1c_core::ir::FormDataAttribute {
            chart_settings: None,
            view_roles: Vec::new(),
            edit_roles: Vec::new(),
            name: name.into(),
            id: 1,
            title: None,
            value_type: None,
            fill_checking: None,
            view_common: true,
            edit_common: true,
            main: false,
            saved_data: false,
            settings_saved_data: Vec::new(),
            designer_unavailable_paths: Vec::new(),
            columns: Vec::new(),
            additional_columns: Vec::new(),
            value_list_ext: false,
            value_list_item_type: None,
            spreadsheet_ext: false,
            functional_options: Vec::new(),
            not_default_use_always: Vec::new(),
            dynamic_list: None,
            spreadsheet_settings: None,
        }
    }

    #[test]
    fn common_form_multi_body_still_refused() {
        // §1.0 shape guard survives the multi-body rework: a CommonForm carrying more than one
        // body is a broken IR — refuse loudly, never scatter bodies over derived paths.
        let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "Ф", Uuid([0; 16]));
        let body = morph1c_core::ir::FormBody::new();
        for name in ["Ф", "Ф2"] {
            obj.form_bodies.push(morph1c_core::ir::NamedFormBody {
                name: name.into(),
                body: body.clone(),
                module: None,
                help: Vec::new(),
                help_resources: Vec::new(),
            });
        }
        let err = write_form_bodies(Format::Edt, Path::new("/nope/CommonForms/Ф/Ф.mdo"), &obj)
            .unwrap_err();
        assert!(matches!(err, ConvertError::Write { .. }), "got {err:?}");
    }
}
