//! READ: байты тела формы → [`FormBody`] (§1.0 тотальность: каждый узел claim'ится или
//! ошибка). Мирроринг `write.rs`; обе формы (EDT/Designer) через [`FormDialect`].

// Shared imports live here in `read/mod.rs`; submodules pull them in via `use super::*`
// (a child glob-import sees the parent module's private `use` aliases too).
use super::fields::{read_fields_designer, read_fields_edt, Region};
use super::projection::read_form_attrs;
use super::tables;
use super::{
    designer_envelope, detect_form_profile, edt_envelope, witnessed_form_versions, FormDialect,
    FormError, CORE_NS_URI, FORM_COMMAND_BAR_NAME, FORM_NS_URI, SCHEMA_NS_URI, SETTINGS_NS_URI,
    XSI_NS_URI,
};
use crate::descriptor::Element;
use crate::value_codec::{self, ValueDialect};
use crate::{parse, type_codec, EolStyle};
use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::ir::{
    AdditionalColumns, AutoCommandBar, ContextMenuBody, DcsAvailableValue, DcsCalculatedField,
    DcsCorValue, DcsField, DcsItem, DcsListSettings, DcsOrderExpression, DcsParamValue, DcsParameter,
    DcsPresentation, DcsRightValue,
    DcsSettingsGroup,
    DcsSettingsParameterValue, DcsUseRestriction, DecoratorBody, DecoratorRef, DocumentFormInfo,
    DynamicListAttrExt, DynamicListExt, FontRef, FormBody, FormCiItem, FormCommand,
    FormControlKind, FormDataAttribute, FormEvent, FormItem, FormParameter, FormRootExtInfo,
    ReportFormInfo, TooltipBody, TypeSpec, REPORT_FORM_AUTO,
};
// `MxlSpreadsheetSettings` — по полному пути модуля (не через re-export `ir::mod`, вне границ лейна).
use morph1c_core::ir::form::{MxlLanguageInfo, MxlLanguageSettings, MxlNode, MxlSpreadsheetSettings};
use morph1c_core::spec::forms::command as fc;
use morph1c_core::spec::forms::controls::label_decoration as ld;
use morph1c_core::spec::forms::controls::table as tb;


mod common;
mod font;
mod mxl;
mod dcs_settings;
mod edt;
mod edt_controls;
mod edt_dcs;
mod designer;
mod designer_controls;
mod designer_dcs;
#[cfg(any())]
mod tests;

pub(crate) use common::*;
pub(crate) use font::*;
pub(crate) use mxl::*;
pub(crate) use dcs_settings::*;
pub(crate) use edt::*;
pub(crate) use edt_controls::*;
pub(crate) use edt_dcs::*;
pub(crate) use designer::*;
pub(crate) use designer_controls::*;
pub(crate) use designer_dcs::*;

/// Прочитать байты тела формы в [`FormBody`] (byte-exact-обратимо).
pub fn read_form(dialect: FormDialect, bytes: &[u8]) -> Result<FormBody, FormError> {
    let descriptor = parse(bytes)?;
    let env = descriptor.bytes_env;
    let want = match dialect {
        FormDialect::Edt => edt_envelope(),
        FormDialect::Designer => designer_envelope(),
    };
    if env.bom != want.bom {
        return Err(FormError::Envelope(format!(
            "BOM mismatch: found {}",
            env.bom
        )));
    }
    if env.eol != EolStyle::Crlf {
        return Err(FormError::Envelope(format!(
            "must be CRLF, found {:?}",
            env.eol
        )));
    }
    match descriptor.decl.as_deref() {
        Some(d) if d == want.decl => {}
        other => return Err(FormError::Envelope(format!("unexpected decl: {other:?}"))),
    }
    let root = descriptor.root;
    match dialect {
        FormDialect::Edt => read_edt(root),
        FormDialect::Designer => read_designer(root),
    }
}

/// Claim корневые ns-атрибуты + сверка. Возвращает ошибку при несоответствии.
pub(super) fn claim_root_ns(root: &Element, want: &[(&str, &str)]) -> Result<(), FormError> {
    for (name, uri) in want {
        let a = root
            .attr(name)
            .ok_or_else(|| FormError::Envelope(format!("missing root {name}")))?;
        if a.value != *uri {
            return Err(FormError::Envelope(format!(
                "root {name}={:?} want {uri:?}",
                a.value
            )));
        }
        a.claimed.set(true);
    }
    Ok(())
}

/// Диагностика §1.0: пути невостребованных узлов дерева (`родитель>ребёнок`, `@attr`,
/// `#text`). Форм-локальный обход (`descriptor.rs` — вне границ этого среза), чтобы
/// сообщение об ошибке тотальности называло КОНКРЕТНЫЙ незакрытый узел, а не только число.
pub(super) fn unclaimed_labels(root: &Element) -> Vec<String> {
    fn tag(el: &Element) -> String {
        if el.prefix.is_empty() {
            el.local.clone()
        } else {
            format!("{}:{}", el.prefix, el.local)
        }
    }
    fn walk(el: &Element, path: &str, out: &mut Vec<String>) {
        if !el.claimed.get() {
            out.push(path.to_string());
        }
        if !el.text.is_empty() && !el.text_claimed.get() {
            out.push(format!("{path}#text"));
        }
        for a in &el.attrs {
            if !a.claimed.get() {
                out.push(format!("{path}@{}", a.name));
            }
        }
        for c in &el.children {
            walk(c, &format!("{path}>{}", tag(c)), out);
        }
    }
    let mut out = Vec::new();
    walk(root, &tag(root), &mut out);
    out
}

