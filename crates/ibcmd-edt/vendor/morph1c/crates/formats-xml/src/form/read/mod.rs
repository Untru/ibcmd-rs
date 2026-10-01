//! READ: байты тела формы → [`FormBody`] (§1.0 тотальность: каждый узел claim'ится или
//! ошибка). Мирроринг `write.rs`; обе формы (EDT/Designer) через [`FormDialect`].

// Shared imports live here in `read/mod.rs`; submodules pull them in via `use super::*`
// (a child glob-import sees the parent module's private `use` aliases too).
use super::fields::{Region, read_fields_designer, read_fields_edt};
use super::projection::read_form_attrs;
use super::tables;
use super::{
    CORE_NS_URI, FORM_COMMAND_BAR_NAME, FORM_NS_URI, FormDialect, FormError, SCHEMA_NS_URI,
    SETTINGS_NS_URI, XSI_NS_URI, designer_envelope, detect_form_profile, edt_envelope,
    witnessed_form_versions,
};
use crate::descriptor::Element;
use crate::value_codec::{self, ValueDialect};
use crate::{EolStyle, parse, type_codec};
use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::ir::{
    AdditionalColumns, AutoCommandBar, ContextMenuBody, DcsAvailableValue, DcsCalculatedField,
    DcsCorValue, DcsField, DcsItem, DcsListSettings, DcsOrderExpression, DcsParamValue,
    DcsParameter, DcsPresentation, DcsRightValue, DcsSettingsGroup, DcsSettingsParameterValue,
    DcsUseRestriction, DecoratorBody, DecoratorRef, DocumentFormInfo, DynamicListAttrExt,
    DynamicListExt, FontRef, FormBody, FormCiItem, FormCommand, FormControlKind, FormDataAttribute,
    FormEvent, FormItem, FormParameter, FormRootExtInfo, REPORT_FORM_AUTO, ReportFormInfo,
    TooltipBody, TypeSpec,
};
// `MxlSpreadsheetSettings` — по полному пути модуля (не через re-export `ir::mod`, вне границ лейна).
use morph1c_core::ir::form::{
    MxlLanguageInfo, MxlLanguageSettings, MxlNode, MxlSpreadsheetSettings,
};
use morph1c_core::spec::forms::command as fc;
use morph1c_core::spec::forms::controls::label_decoration as ld;
use morph1c_core::spec::forms::controls::table as tb;

mod common;
mod dcs_settings;
mod designer;
mod designer_controls;
mod designer_dcs;
mod edt;
mod edt_controls;
mod edt_dcs;
mod font;
mod mxl;
#[cfg(any())]
mod tests;

pub(crate) use common::*;
pub(crate) use dcs_settings::*;
pub(crate) use designer::*;
pub(crate) use designer_controls::*;
pub(crate) use designer_dcs::*;
pub(crate) use edt::*;
pub(crate) use edt_controls::*;
pub(crate) use edt_dcs::*;
pub(crate) use font::*;
pub(crate) use mxl::*;

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
    validate_qname_bindings(&root, &std::collections::BTreeMap::new())?;
    let mut body = match dialect {
        FormDialect::Edt => read_edt(&root),
        FormDialect::Designer => {
            let mut presence = std::collections::BTreeMap::new();
            if root
                .attr("version")
                .is_some_and(|attribute| attribute.value == "2.20")
            {
                collect_xml220_checkbox_presence(&root, &mut presence)?;
            }
            let mut body = read_designer(&root)?;
            body.designer_checkbox_auto_presence = presence;
            body.designer_path_spelling = true;
            Ok(body)
        }
    }?;
    // Both readers have already rejected every unclaimed node/attribute.
    body.source_wire_order = Some(super::wire_order::capture(dialect, &root, &body)?);
    Ok(body)
}

fn collect_xml220_checkbox_presence(
    element: &Element,
    out: &mut std::collections::BTreeMap<i64, bool>,
) -> Result<(), FormError> {
    if element.prefix.is_empty() && element.local == "CheckBoxField" {
        let ty = element
            .children
            .iter()
            .find(|child| child.prefix.is_empty() && child.local == "CheckBoxType");
        if ty.is_none_or(|ty| ty.text == "Auto") {
            let id = element
                .attr("id")
                .ok_or_else(|| FormError::Frame("checkbox has no id".into()))?
                .value
                .parse::<i64>()
                .map_err(|_| FormError::Frame("checkbox id is not an integer".into()))?;
            if out.insert(id, ty.is_some()).is_some() {
                return Err(FormError::Frame(
                    "duplicate checkbox id in spelling facet".into(),
                ));
            }
        }
    }
    for child in &element.children {
        collect_xml220_checkbox_presence(child, out)?;
    }
    Ok(())
}

fn validate_qname_bindings(
    element: &Element,
    inherited: &std::collections::BTreeMap<String, String>,
) -> Result<(), FormError> {
    let mut namespaces = inherited.clone();
    for attribute in &element.attrs {
        if let Some(prefix) = attribute.name.strip_prefix("xmlns:") {
            namespaces.insert(prefix.into(), attribute.value.clone());
        }
    }
    if let Some(attribute) = element.attr("xsi:type") {
        if namespaces.get("xsi").map(String::as_str) != Some(XSI_NS_URI) {
            return Err(FormError::Envelope(
                "xsi:type requires the XML Schema instance namespace".into(),
            ));
        }
        if let Some((prefix, local)) = attribute.value.split_once(':') {
            if local.is_empty() || !namespaces.get(prefix).is_some_and(|uri| !uri.is_empty()) {
                return Err(FormError::Envelope("xsi:type uses an unbound QName".into()));
            }
        }
    }
    for child in &element.children {
        validate_qname_bindings(child, &namespaces)?;
    }
    Ok(())
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
