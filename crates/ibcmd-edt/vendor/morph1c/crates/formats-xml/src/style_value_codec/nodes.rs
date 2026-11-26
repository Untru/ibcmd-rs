//! Хелперы обхода узлов (EDT-дети / Designer-атрибуты) + общие парсеры/валидаторы (§1.0-total).

use super::*;

// ============================================================================
// Хелперы обхода EDT-детей (строго-последовательный, §1.0-total)
// ============================================================================

/// Последовательный итератор непосредственных детей EDT inner-value: читает поля в
/// фиксированном порядке, требуя, что каждый прочитанный ребёнок — plain-text лист без
/// атрибутов/детей; `finish` гарантирует отсутствие лишних детей (§1.0).
pub(crate) struct EdtChildren<'a> {
    children: &'a [Element],
    pos: usize,
}

impl<'a> EdtChildren<'a> {
    pub(crate) fn new(inner: &'a Element) -> Self {
        EdtChildren {
            children: &inner.children,
            pos: 0,
        }
    }

    /// Забрать текстовый лист поля `name`, если он следующий. Проверяет, что это
    /// plain-text лист (без атрибутов/детей), claim'ит его.
    pub(crate) fn opt_text(&mut self, name: &str) -> Result<Option<String>, String> {
        match self.children.get(self.pos) {
            Some(c) if c.local == name && c.prefix.is_empty() => {
                if !c.attrs.is_empty() || !c.children.is_empty() {
                    return Err(format!(
                        "StyleValue: EDT <{name}> must be a plain-text leaf (§1.0)"
                    ));
                }
                c.claim_with_text();
                self.pos += 1;
                Ok(Some(c.text.clone()))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn require_text(&mut self, name: &str) -> Result<String, String> {
        self.opt_text(name)?
            .ok_or_else(|| format!("StyleValue: EDT expects <{name}> here (§1.0)"))
    }

    pub(crate) fn opt_u32(&mut self, name: &str) -> Result<Option<u32>, String> {
        match self.opt_text(name)? {
            Some(t) => Ok(Some(parse_u32(name, &t)?)),
            None => Ok(None),
        }
    }
    pub(crate) fn opt_bool(&mut self, name: &str) -> Result<Option<bool>, String> {
        match self.opt_text(name)? {
            Some(t) => Ok(Some(parse_bool(name, &t)?)),
            None => Ok(None),
        }
    }
    /// RGB-компонента: опциональный лист `red`/`green`/`blue`, значение 0..=255; отсутствие → 0.
    pub(crate) fn opt_component(&mut self, name: &str) -> Result<u8, String> {
        match self.opt_text(name)? {
            Some(t) => parse_u8(name, &t),
            None => Ok(0),
        }
    }

    /// EDT height: `N.0` (целое с обязательным `.0`).
    pub(crate) fn require_edt_height(&mut self, name: &str) -> Result<u32, String> {
        let t = self.require_text(name)?;
        parse_edt_height(name, &t)
    }
    pub(crate) fn opt_edt_height(&mut self, name: &str) -> Result<Option<u32>, String> {
        match self.opt_text(name)? {
            Some(t) => Ok(Some(parse_edt_height(name, &t)?)),
            None => Ok(None),
        }
    }

    /// Все дети прочитаны (иначе лишний ребёнок → ошибка, §1.0).
    pub(crate) fn finish(self) -> Result<(), String> {
        if let Some(extra) = self.children.get(self.pos) {
            return Err(format!(
                "StyleValue: EDT unexpected extra child <{}> in style value (§1.0)",
                extra.local
            ));
        }
        Ok(())
    }
}

// ============================================================================
// Хелперы обхода Designer-атрибутов (строго-total)
// ============================================================================

/// Обходчик атрибутов Designer `<Value>`: читает по имени, помечает claimed, `finish`
/// гарантирует отсутствие непрочитанных атрибутов (§1.0).
pub(crate) struct DesAttrs<'a> {
    host: &'a Element,
}

impl<'a> DesAttrs<'a> {
    pub(crate) fn new(host: &'a Element) -> Self {
        DesAttrs { host }
    }
    pub(crate) fn claim(&mut self, name: &str) {
        if let Some(a) = self.host.attr(name) {
            a.claimed.set(true);
        }
    }
    pub(crate) fn opt(&mut self, name: &str) -> Option<String> {
        let a = self.host.attr(name)?;
        a.claimed.set(true);
        Some(a.value.clone())
    }
    pub(crate) fn require(&mut self, name: &str) -> Result<String, String> {
        self.opt(name)
            .ok_or_else(|| format!("StyleValue: Designer Font missing @{name} (§1.0)"))
    }
    pub(crate) fn opt_u32(&mut self, name: &str) -> Result<Option<u32>, String> {
        match self.opt(name) {
            Some(v) => Ok(Some(parse_u32(name, &v)?)),
            None => Ok(None),
        }
    }
    pub(crate) fn require_u32(&mut self, name: &str) -> Result<u32, String> {
        let v = self.require(name)?;
        parse_u32(name, &v)
    }
    pub(crate) fn require_bool(&mut self, name: &str) -> Result<bool, String> {
        let v = self.require(name)?;
        parse_bool(name, &v)
    }
    pub(crate) fn opt_bool(&mut self, name: &str) -> Result<Option<bool>, String> {
        match self.opt(name) {
            Some(v) => Ok(Some(parse_bool(name, &v)?)),
            None => Ok(None),
        }
    }
    pub(crate) fn finish(self) -> Result<(), String> {
        if let Some(extra) = self.host.attrs.iter().find(|a| !a.claimed.get()) {
            return Err(format!(
                "StyleValue: Designer Value has unexpected attribute {:?} (§1.0)",
                extra.name
            ));
        }
        Ok(())
    }
}

// ============================================================================
// Общие парсеры/валидаторы узлов
// ============================================================================

/// Забрать единственный `xsi:type`, claim его, ошибиться на любом другом атрибуте (§1.0).
pub(crate) fn claim_only_xsi(el: &Element) -> Result<String, String> {
    let a = el
        .attr(XSI_TYPE)
        .ok_or_else(|| format!("StyleValue: <{}> missing @{XSI_TYPE} (§1.0)", el.local))?;
    a.claimed.set(true);
    if let Some(extra) = el.attrs.iter().find(|x| !x.claimed.get()) {
        return Err(format!(
            "StyleValue: <{}> has unexpected attribute {:?} (§1.0)",
            el.local, extra.name
        ));
    }
    Ok(a.value.clone())
}

pub(crate) fn require_attr(el: &Element, name: &str) -> Result<String, String> {
    el.attr(name)
        .map(|a| a.value.clone())
        .ok_or_else(|| format!("StyleValue: <{}> missing @{name} (§1.0)", el.local))
}

/// Единственный ребёнок с данным local/prefix; claim его самого. Ошибка на 0/2+/чужой.
pub(crate) fn single_child<'a>(
    host: &'a Element,
    local: &str,
    prefix: &str,
) -> Result<&'a Element, String> {
    let mut it = host.children.iter();
    let child = match it.next() {
        Some(c) if c.local == local && c.prefix == prefix => c,
        Some(c) => {
            return Err(format!(
                "StyleValue: EDT expects single <{local}> child, got <{}> (§1.0)",
                c.local
            ))
        }
        None => return Err(format!("StyleValue: EDT missing <{local}> child (§1.0)")),
    };
    if it.next().is_some() {
        return Err(format!(
            "StyleValue: EDT expects EXACTLY one <{local}> child (§1.0)"
        ));
    }
    child.claim();
    Ok(child)
}

pub(crate) fn ensure_no_children_des(host: &Element) -> Result<(), String> {
    if let Some(c) = host.children.first() {
        return Err(format!(
            "StyleValue: Designer <Value> must have no child elements, got <{}> (§1.0)",
            c.local
        ));
    }
    Ok(())
}

fn parse_u32(name: &str, t: &str) -> Result<u32, String> {
    t.parse::<u32>()
        .map_err(|_| format!("StyleValue: {name} must be a non-negative integer, got {t:?} (§1.0)"))
}
fn parse_u8(name: &str, t: &str) -> Result<u8, String> {
    t.parse::<u8>()
        .map_err(|_| format!("StyleValue: {name} must be 0..=255, got {t:?} (§1.0)"))
}
fn parse_bool(name: &str, t: &str) -> Result<bool, String> {
    match t {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "StyleValue: {name} must be true/false, got {other:?} (§1.0)"
        )),
    }
}
/// EDT height — целое с обязательным суффиксом `.0` (`14.0`). Иная форма → ошибка (§1.0).
fn parse_edt_height(name: &str, t: &str) -> Result<u32, String> {
    let base = t.strip_suffix(".0").ok_or_else(|| {
        format!(
            "StyleValue: EDT {name} must be integer with .0 suffix (e.g. 14.0), got {t:?} (§1.0)"
        )
    })?;
    parse_u32(name, base)
}
