//! Хелперы: канонизация ссылок / формат числа / bool.

use super::*;

// ============================================================================
// Хелперы: канонизация ссылок / формат числа / bool
// ============================================================================

/// EDT height: целое → `N.0` (конвенция EDT `.mdo`: `<height>14.0</height>`).
pub(crate) fn edt_height(h: u32) -> String {
    format!("{h}.0")
}

pub(crate) fn bool_text(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// FontRef канон = EDT-форма `Prefix.Имя` (уже канон). Валидируем префикс по семействам
/// [`FONT_REF_FAMILIES`] (`Style.` стиль-ссылка / `System.` системный шрифт).
pub(crate) fn font_ref_canon_from_edt(edt: &str) -> Result<String, String> {
    let (pref, _name) = split_dot(edt)?;
    if !FONT_REF_FAMILIES.iter().any(|(e, _, _)| *e == pref) {
        return Err(format!(
            "StyleValue: EDT FontRef must be Style.<Name>/System.<Name>, got {edt:?} (§1.0)"
        ));
    }
    Ok(edt.to_string())
}
pub(crate) fn font_ref_edt_from_canon(canon: &str) -> String {
    canon.to_string()
}
/// Designer `@ref` → канон, с КРОСС-СВЕРКОЙ против `@kind`: префикс ref обязан принадлежать
/// РОВНО тому семейству, что заявлен kind'ом (`style:`↔StyleItem, `sys:`↔WindowsFont —
/// witnessed попарно, рассинхрон = невитнессированная форма → ошибка, §1.0).
pub(crate) fn font_ref_canon_from_designer(des: &str, kind: &str) -> Result<String, String> {
    let (pref, name) = split_colon(des)?;
    let family = FONT_REF_FAMILIES
        .iter()
        .find(|(_, d, _)| *d == pref)
        .ok_or_else(|| {
            format!("StyleValue: Designer Font ref must be style:<Name>/sys:<Name>, got {des:?} (§1.0)")
        })?;
    if family.2 != kind {
        return Err(format!(
            "StyleValue: Designer Font ref {des:?} does not match kind {kind:?} \
             (witnessed pairs: style:↔StyleItem, sys:↔WindowsFont — §1.0)"
        ));
    }
    Ok(format!("{}.{name}", family.0))
}
/// Канон → `(designer-@ref, designer-@kind)` по семейству префикса канона.
pub(crate) fn font_ref_des_from_canon(canon: &str) -> (String, &'static str) {
    if let Ok((pref, name)) = split_dot(canon) {
        if let Some((_, d, kind)) = FONT_REF_FAMILIES.iter().find(|(e, _, _)| *e == pref) {
            return (format!("{d}:{name}"), kind);
        }
    }
    // Недостижимо для валидного канона (префикс валидирован на decode обоих диалектов).
    (canon.to_string(), DES_KIND_REF)
}

/// ColorRef канон = EDT-форма `Prefix.Имя`. Валидируем известный префикс.
pub(crate) fn color_ref_validate(edt: &str) -> Result<(), String> {
    let (pref, _name) = split_dot(edt)?;
    if !COLOR_PREFIXES.iter().any(|(e, _)| *e == pref) {
        return Err(format!(
            "StyleValue: EDT ColorRef prefix must be Palette/Web/Style, got {edt:?} (§1.0)"
        ));
    }
    Ok(())
}
pub(crate) fn color_ref_canon_from_designer(des: &str) -> Result<String, String> {
    let (pref, name) = split_colon(des)?;
    let edt_pref = COLOR_PREFIXES
        .iter()
        .find(|(_, d)| *d == pref)
        .map(|(e, _)| *e)
        .ok_or_else(|| {
            format!(
                "StyleValue: Designer Color ref prefix must be pal/web/style, got {des:?} (§1.0)"
            )
        })?;
    Ok(format!("{edt_pref}.{name}"))
}
pub(crate) fn color_ref_des_from_canon(canon: &str) -> String {
    if let Ok((pref, name)) = split_dot(canon) {
        if let Some((_, d)) = COLOR_PREFIXES.iter().find(|(e, _)| *e == pref) {
            return format!("{d}:{name}");
        }
    }
    canon.to_string() // недостижимо для валидного канона (валидирован на decode).
}

fn split_dot(s: &str) -> Result<(&str, &str), String> {
    s.split_once('.')
        .ok_or_else(|| format!("StyleValue: expected `Prefix.Name`, got {s:?} (§1.0)"))
}
fn split_colon(s: &str) -> Result<(&str, &str), String> {
    s.split_once(':')
        .ok_or_else(|| format!("StyleValue: expected `prefix:Name`, got {s:?} (§1.0)"))
}
