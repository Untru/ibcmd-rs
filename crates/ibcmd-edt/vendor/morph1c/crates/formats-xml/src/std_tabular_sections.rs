//! ОБОБЩЁННЫЙ (data-driven) кодек платформенного блока `standardTabularSections`/
//! `StandardTabularSections` (субстрат стандартных ТЧ; носители — ChartOfAccounts и
//! ChartOfCalculationTypes). Читает `&'static StdTsDecl` из канонического спека
//! (`core/spec`) и проецирует его в ОБА XML-формата (§1.6: канон — в спеке, формат —
//! лишь проекция; новый вид-носитель добавляет ТОЛЬКО декларацию в свой спек-файл).
//!
//! Набор ТЧ у вида ФИКСИРОВАН платформой (CoA — 1 `ExtDimensionTypes`; CCalcT — 3
//! Leading/Displacing/BaseCalculationTypes; ERP-witnessed оба вида × оба объекта):
//! блок либо ОПУЩЕН ЦЕЛИКОМ (все ТЧ дефолтны — coverage/SSL), либо несёт ВСЕ ТЧ
//! набора в каноническом порядке; иной счётчик — §1.0-ошибка. Имя ТЧ в IR не
//! хранится (следует из позиции в декларации); read СВЕРЯЕТ наблюдаемое имя.
//!
//! # Форма (ERP-witnessed, оба вида)
//! EDT — подряд блоки от КОРНЯ `.mdo`:
//! ```text
//! <standardTabularSections>
//!   <name>ИмяТЧ</name>
//!   [<synonym><key>lang</key><value>текст</value></synonym>…]   ← multi-sibling;
//!   [<comment>…</comment>] [<toolTip>…</toolTip>…]                 witnessed ПУСТОЙ lang
//!   [<fillChecking>ShowError</fillChecking>]                       (<key></key>)
//!   <standardAttributes>…</standardAttributes> ×K                ← вложенный std-attrs,
//! </standardTabularSections>                                       грамматика decl.attrs
//! ```
//! Designer — обёртка `<root_elem>/<Properties>/<StandardTabularSections>`:
//! ```text
//! <xr:StandardTabularSection name="ИмяТЧ">
//!   <xr:Synonym><v8:item><v8:lang/><v8:content>текст</v8:content></v8:item></xr:Synonym>
//!   <xr:Comment/> <xr:ToolTip/> <xr:FillChecking>DontCheck</xr:FillChecking>
//!   <xr:StandardAttributes><xr:StandardAttribute name="…">×K</xr:StandardAttributes>
//! </xr:StandardTabularSection>
//! ```
//! ⚠️ ПУСТОЙ язык синонима — witnessed-инвариант обоих диалектов: EDT несёт
//! `<key></key>` (НЕ самозакрытый), Designer — `<v8:lang/>` (самозакрытый) — поэтому
//! designer-эмиссия локализации здесь СВОЯ ([`emit_localized_v8_sts`]), а не общая
//! (общая пишет `<v8:lang></v8:lang>`).
//!
//! # Канонический IR (см. `spec::common::StdTsDecl`)
//! `List` записей-ТЧ; запись — `List` из 5 значений:
//! `[synonym: Localized, comment: Str, toolTip: Localized, fillChecking: Enum(DontCheck),
//!   standardAttributes: List]`; вложенный список — записи по грамматике `decl.attrs`
//! (машинерия атрибут-уровня — ВЕРБАТИМ [`crate::std_attrs_generic`], те же
//! variable-слоты → X by construction с cf-стороной `formats_brace::std_tabular_sections`).

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::std_attrs_generic::{self, GenDialect};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::spec::common::{StdTsDecl, StdTsSection};
use morph1c_core::version::FormatVersion;

// --- слоты записи-ТЧ (канонический порядок; см. StdTsDecl doc) ---
const TS_SYNONYM: usize = 0;
const TS_COMMENT: usize = 1;
const TS_TOOLTIP: usize = 2;
const TS_FILL_CHECKING: usize = 3;
const TS_STD_ATTRS: usize = 4;
const N_TS_VAR: usize = 5;

/// Дефолт-литерал `fillChecking` самой ТЧ (омитится в EDT; witnessed 8/8 ТЧ).
const TS_FILL_CHECKING_DEFAULT: &str = "DontCheck";

fn default_ts_record() -> Vec<PropertyValue> {
    vec![
        PropertyValue::Localized(Vec::new()),                    // synonym
        PropertyValue::Str(String::new()),                       // comment
        PropertyValue::Localized(Vec::new()),                    // toolTip
        PropertyValue::Enum(Token::new(TS_FILL_CHECKING_DEFAULT)), // fillChecking
        PropertyValue::List(Vec::new()),                         // standardAttributes
    ]
}

/// Распаковать IR-`List` поля в записи-ТЧ: 0 записей (блок опущен) либо ровно по
/// одной на секцию декларации (§1.0: иное — ошибка).
fn unpack_ts<'a>(
    decl: &StdTsDecl,
    value: &'a PropertyValue,
) -> Result<Vec<&'a Vec<PropertyValue>>, String> {
    let outer = match value {
        PropertyValue::List(v) => v,
        other => {
            return Err(format!(
                "{} standardTabularSections IR must be List, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    };
    if outer.is_empty() {
        return Ok(Vec::new());
    }
    if outer.len() != decl.sections.len() {
        return Err(format!(
            "{} standardTabularSections: expected 0 or {} sections, got {} (fixed platform \
             set — §1.0)",
            decl.label,
            decl.sections.len(),
            outer.len()
        ));
    }
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        match item {
            PropertyValue::List(rec) if rec.len() == N_TS_VAR => out.push(rec),
            _ => {
                return Err(format!(
                    "{} standardTabularSections: each section must be List of {N_TS_VAR}",
                    decl.label
                ))
            }
        }
    }
    Ok(out)
}

// ---- public DECODE / CLAIM / EMIT ------------------------------------------

/// Декодировать блок (от source-root) в канонический IR-`List`. Строго-тотально.
pub fn decode(
    dialect: GenDialect,
    decl: &StdTsDecl,
    root: &Element,
    version: FormatVersion,
) -> Decoded {
    let res = match dialect {
        GenDialect::Edt => decode_edt(decl, root),
        GenDialect::Designer => decode_designer(decl, root, version),
    };
    match res {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

/// Claim-проход (для leftover): клеймит блок РОВНО если он совпал с ожидаемой структурой.
pub fn claim(dialect: GenDialect, decl: &StdTsDecl, root: &Element, version: FormatVersion) {
    match dialect {
        GenDialect::Edt => {
            let _ = decode_edt(decl, root);
        }
        GenDialect::Designer => {
            let _ = decode_designer(decl, root, version);
        }
    }
}

/// Эмитировать блок byte-exact из IR-`List`. Пустой список ⇒ ноль узлов (блок опущен).
pub fn emit(
    dialect: GenDialect,
    decl: &StdTsDecl,
    value: &PropertyValue,
    version: FormatVersion,
) -> Result<Vec<OutElement>, String> {
    match dialect {
        GenDialect::Edt => emit_edt(decl, value),
        GenDialect::Designer => emit_designer(decl, value, version),
    }
}

// ---- EDT --------------------------------------------------------------------

fn decode_edt(decl: &StdTsDecl, root: &Element) -> Result<PropertyValue, String> {
    let blocks: Vec<&Element> = root
        .children
        .iter()
        .filter(|c| c.local == "standardTabularSections" && c.prefix.is_empty())
        .collect();
    if blocks.is_empty() {
        // Блок опционален ЦЕЛИКОМ (все ТЧ дефолтны — coverage/SSL) → пустой IR-List.
        return Ok(PropertyValue::List(Vec::new()));
    }
    if blocks.len() != decl.sections.len() {
        return Err(format!(
            "{} standardTabularSections: found {} blocks, platform set holds {} (all-or-none \
             — §1.0)",
            decl.label,
            blocks.len(),
            decl.sections.len()
        ));
    }
    let mut records = Vec::with_capacity(blocks.len());
    for (block, section) in blocks.iter().zip(decl.sections) {
        records.push(decode_edt_ts(decl, block, section)?);
        block.claim();
    }
    Ok(PropertyValue::List(
        records.into_iter().map(PropertyValue::List).collect(),
    ))
}

fn decode_edt_ts(
    decl: &StdTsDecl,
    block: &Element,
    section: &StdTsSection,
) -> Result<Vec<PropertyValue>, String> {
    if !block.attrs.is_empty() {
        return Err(format!(
            "{} standardTabularSections block must have no attributes",
            decl.label
        ));
    }
    let mut rec = default_ts_record();
    let mut it = block.children.iter().peekable();
    // <name> — обязательный первый лист; имя СВЕРЯЕТСЯ с декларацией (набор фикс).
    let name_el = it
        .next()
        .filter(|c| c.local == "name" && c.prefix.is_empty())
        .ok_or_else(|| {
            format!(
                "{} standardTabularSections: expected <name> first (§1.0)",
                decl.label
            )
        })?;
    if !name_el.attrs.is_empty() || !name_el.children.is_empty() || name_el.text != section.name {
        return Err(format!(
            "{} standardTabularSections: expected section {:?}, got {:?} (canonical platform \
             order — §1.0)",
            decl.label, section.name, name_el.text
        ));
    }
    name_el.claim_with_text();
    // Собственные листья ТЧ — SPARSE, позиционный порядок = Designer-DENSE
    // (Synonym/Comment/ToolTip/FillChecking; witnessed только name+synonym).
    std_attrs_generic::take_opt_localized(&mut it, "synonym", &mut rec[TS_SYNONYM])?;
    std_attrs_generic::take_opt_plain(&mut it, "comment", &mut rec[TS_COMMENT])?;
    std_attrs_generic::take_opt_localized(&mut it, "toolTip", &mut rec[TS_TOOLTIP])?;
    std_attrs_generic::take_opt_enum(&mut it, "fillChecking", &mut rec[TS_FILL_CHECKING])?;
    // Вложенный std-attrs-блок: РОВНО по одному <standardAttributes> на атрибут
    // фикс-набора (грамматика листьев — decl секции; машинерия — std_attrs_generic).
    let attrs = section.attrs.root_attrs;
    let mut nested = Vec::with_capacity(attrs.len());
    for attr in attrs {
        let b = it
            .next()
            .filter(|c| c.local == "standardAttributes" && c.prefix.is_empty())
            .ok_or_else(|| {
                format!(
                    "{} standardTabularSections[{}]: expected <standardAttributes> for {} \
                     (fixed set of {} — §1.0)",
                    decl.label,
                    section.name,
                    attr.name,
                    attrs.len()
                )
            })?;
        nested.push(std_attrs_generic::decode_edt_block(section.attrs, b, attr)?);
        b.claim();
    }
    rec[TS_STD_ATTRS] = std_attrs_generic::pack(nested);
    if let Some(extra) = it.next() {
        return Err(format!(
            "{} standardTabularSections[{}]: unexpected extra leaf <{}> (§1.0)",
            decl.label, section.name, extra.local
        ));
    }
    Ok(rec)
}

fn emit_edt(decl: &StdTsDecl, value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let records = unpack_ts(decl, value)?;
    let mut out = Vec::with_capacity(records.len());
    for (section, rec) in decl.sections.iter().zip(&records) {
        out.push(emit_edt_ts(decl, section, rec)?);
    }
    Ok(out)
}

fn emit_edt_ts(
    decl: &StdTsDecl,
    section: &StdTsSection,
    rec: &[PropertyValue],
) -> Result<OutElement, String> {
    let mut block = OutElement::branch("", "standardTabularSections");
    block.push(OutElement::leaf("", "name", section.name.to_string()));
    // synonym/toolTip — multi-sibling key/val (пустой lang даёт `<key></key>` — leaf с
    // пустым текстом рендерится НЕ-самозакрытым, byte-exact witnessed-форме).
    for el in std_attrs_generic::emit_localized_keyval("synonym", &rec[TS_SYNONYM])? {
        block.push(el);
    }
    match &rec[TS_COMMENT] {
        PropertyValue::Str(s) if s.is_empty() => {}
        PropertyValue::Str(s) => block.push(OutElement::leaf("", "comment", s.clone())),
        other => {
            return Err(format!(
                "{} standardTabularSections comment must be Str, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    }
    for el in std_attrs_generic::emit_localized_keyval("toolTip", &rec[TS_TOOLTIP])? {
        block.push(el);
    }
    match &rec[TS_FILL_CHECKING] {
        PropertyValue::Enum(t) if t.as_str() == TS_FILL_CHECKING_DEFAULT => {}
        PropertyValue::Enum(t) => {
            block.push(OutElement::leaf("", "fillChecking", t.as_str().to_string()))
        }
        other => {
            return Err(format!(
                "{} standardTabularSections fillChecking must be Enum, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    }
    let attrs = section.attrs.root_attrs;
    let nested = std_attrs_generic::unpack(section.attrs, &rec[TS_STD_ATTRS], attrs.len())?;
    for (attr, nrec) in attrs.iter().zip(&nested) {
        block.push(std_attrs_generic::emit_edt_block(section.attrs, attr, nrec)?);
    }
    Ok(block)
}

// ---- Designer ---------------------------------------------------------------

/// Найти Designer-обёртку `<StandardTabularSections>` (под `<root_elem>/<Properties>`).
fn designer_wrapper<'a>(decl: &StdTsDecl, root: &'a Element) -> Option<&'a Element> {
    root.child(decl.root_elem)?
        .child("Properties")?
        .child("StandardTabularSections")
}

fn decode_designer(
    decl: &StdTsDecl,
    root: &Element,
    version: FormatVersion,
) -> Result<PropertyValue, String> {
    let wrapper = match designer_wrapper(decl, root) {
        Some(w) => w,
        // Блок опционален целиком → пустой IR-List (зеркало EDT/cf-`{0}`).
        None => return Ok(PropertyValue::List(Vec::new())),
    };
    if !wrapper.prefix.is_empty() || !wrapper.attrs.is_empty() || !wrapper.text.is_empty() {
        return Err(format!(
            "{} <StandardTabularSections> must be an unprefixed attribute-less container",
            decl.label
        ));
    }
    if wrapper.children.len() != decl.sections.len() {
        return Err(format!(
            "{} <StandardTabularSections>: found {} sections, platform set holds {} \
             (all-or-none — §1.0)",
            decl.label,
            wrapper.children.len(),
            decl.sections.len()
        ));
    }
    let mut records = Vec::with_capacity(decl.sections.len());
    for (ts_el, section) in wrapper.children.iter().zip(decl.sections) {
        records.push(decode_designer_ts(decl, ts_el, section, version)?);
        ts_el.claim();
    }
    wrapper.claim();
    Ok(PropertyValue::List(
        records.into_iter().map(PropertyValue::List).collect(),
    ))
}

fn decode_designer_ts(
    decl: &StdTsDecl,
    ts_el: &Element,
    section: &StdTsSection,
    version: FormatVersion,
) -> Result<Vec<PropertyValue>, String> {
    if ts_el.local != "StandardTabularSection" || ts_el.prefix != "xr" {
        return Err(format!(
            "{} standardTabularSections: expected <xr:StandardTabularSection>, got <{}>",
            decl.label, ts_el.local
        ));
    }
    if ts_el.attrs.len() != 1
        || ts_el.attrs[0].name != "name"
        || ts_el.attrs[0].value != *section.name
    {
        return Err(format!(
            "{} <xr:StandardTabularSection>: expected single name={:?} (canonical platform \
             order — §1.0)",
            decl.label, section.name
        ));
    }
    ts_el.attrs[0].claimed.set(true);
    // DENSE-регион ТЧ: РОВНО 4 собственных листа + вложенный <xr:StandardAttributes>
    // (witnessed 8/8 ТЧ ERP designer 8.3.27).
    if ts_el.children.len() != 5 {
        return Err(format!(
            "{} <xr:StandardTabularSection name={:?}>: expected 5 children \
             (Synonym/Comment/ToolTip/FillChecking/StandardAttributes), found {}",
            decl.label,
            section.name,
            ts_el.children.len()
        ));
    }
    let mut rec = default_ts_record();
    rec[TS_SYNONYM] =
        std_attrs_generic::take_localized_v8(expect_xr_child(decl, section, ts_el, 0, "Synonym")?)?;
    rec[TS_COMMENT] =
        std_attrs_generic::take_plain_v8(expect_xr_child(decl, section, ts_el, 1, "Comment")?)?;
    rec[TS_TOOLTIP] =
        std_attrs_generic::take_localized_v8(expect_xr_child(decl, section, ts_el, 2, "ToolTip")?)?;
    {
        let fc = expect_xr_child(decl, section, ts_el, 3, "FillChecking")?;
        if !fc.attrs.is_empty() || !fc.children.is_empty() {
            return Err(format!(
                "{} <xr:FillChecking> must be a text leaf",
                decl.label
            ));
        }
        fc.claim_with_text();
        rec[TS_FILL_CHECKING] = PropertyValue::Enum(Token::new(fc.text.clone()));
    }
    let sa = expect_xr_child(decl, section, ts_el, 4, "StandardAttributes")?;
    if !sa.attrs.is_empty() || !sa.text.is_empty() {
        return Err(format!(
            "{} nested <xr:StandardAttributes> must be an attribute-less container",
            decl.label
        ));
    }
    let attrs = section.attrs.root_attrs;
    if sa.children.len() != attrs.len() {
        return Err(format!(
            "{} <xr:StandardTabularSection name={:?}>: nested StandardAttributes holds {} \
             of expected {} (fixed set — §1.0)",
            decl.label,
            section.name,
            sa.children.len(),
            attrs.len()
        ));
    }
    let leaves = std_attrs_generic::dense_leaves_for(section.attrs, version);
    let mut nested = Vec::with_capacity(attrs.len());
    for (attr_el, attr) in sa.children.iter().zip(attrs) {
        nested.push(std_attrs_generic::decode_designer_attr(
            section.attrs,
            attr_el,
            attr,
            &leaves,
        )?);
        attr_el.claim();
    }
    sa.claim();
    rec[TS_STD_ATTRS] = std_attrs_generic::pack(nested);
    Ok(rec)
}

/// Позиционный ребёнок `<xr:StandardTabularSection>` с сверкой local-name/префикса
/// (DENSE-регион ТЧ фиксирован; §1.0).
fn expect_xr_child<'a>(
    decl: &StdTsDecl,
    section: &StdTsSection,
    ts_el: &'a Element,
    i: usize,
    local: &str,
) -> Result<&'a Element, String> {
    let c = &ts_el.children[i];
    if c.local != local || c.prefix != "xr" {
        return Err(format!(
            "{} <xr:StandardTabularSection name={:?}>: expected <xr:{local}> at #{i}, got <{}>",
            decl.label, section.name, c.local
        ));
    }
    Ok(c)
}

fn emit_designer(
    decl: &StdTsDecl,
    value: &PropertyValue,
    version: FormatVersion,
) -> Result<Vec<OutElement>, String> {
    let records = unpack_ts(decl, value)?;
    // Блок опущен целиком: обёртка НЕ эмитится (coverage/SSL byte-exact).
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let mut wrapper = OutElement::branch("", "StandardTabularSections");
    for (section, rec) in decl.sections.iter().zip(&records) {
        wrapper.push(emit_designer_ts(decl, section, rec, version)?);
    }
    Ok(vec![wrapper])
}

fn emit_designer_ts(
    decl: &StdTsDecl,
    section: &StdTsSection,
    rec: &[PropertyValue],
    version: FormatVersion,
) -> Result<OutElement, String> {
    let mut ts_el =
        OutElement::branch("xr", "StandardTabularSection").attr("name", section.name);
    ts_el.push(emit_localized_v8_sts("Synonym", &rec[TS_SYNONYM])?);
    ts_el.push(std_attrs_generic::emit_plain_v8("Comment", &rec[TS_COMMENT])?);
    ts_el.push(emit_localized_v8_sts("ToolTip", &rec[TS_TOOLTIP])?);
    match &rec[TS_FILL_CHECKING] {
        PropertyValue::Enum(t) => {
            ts_el.push(OutElement::leaf("xr", "FillChecking", t.as_str().to_string()))
        }
        other => {
            return Err(format!(
                "{} standardTabularSections fillChecking must be Enum, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    }
    let attrs = section.attrs.root_attrs;
    let nested = std_attrs_generic::unpack(section.attrs, &rec[TS_STD_ATTRS], attrs.len())?;
    let leaves = std_attrs_generic::dense_leaves_for(section.attrs, version);
    let mut sa = OutElement::branch("xr", "StandardAttributes");
    for (attr, nrec) in attrs.iter().zip(&nested) {
        sa.push(std_attrs_generic::emit_designer_attr(attr, nrec, &leaves)?);
    }
    ts_el.push(sa);
    Ok(ts_el)
}

/// Designer localized emit собственных листьев ТЧ: как общий `<xr:Tag>/<v8:item>`,
/// но ПУСТОЙ язык эмитится САМОЗАКРЫТЫМ `<v8:lang/>` — witnessed-форма синонимов
/// стандартных ТЧ (общий emit дал бы `<v8:lang></v8:lang>` — не byte-exact).
fn emit_localized_v8_sts(tag: &str, v: &PropertyValue) -> Result<OutElement, String> {
    let pairs = match v {
        PropertyValue::Localized(p) => p,
        other => {
            return Err(format!(
                "standardTabularSections localized field must be Localized, got {:?}",
                other.kind()
            ))
        }
    };
    if pairs.is_empty() {
        return Ok(OutElement::self_closing("xr", tag));
    }
    let mut c = OutElement::branch("xr", tag);
    for (lang, content) in pairs {
        let mut item = OutElement::branch("v8", "item");
        if lang.as_str().is_empty() {
            item.push(OutElement::self_closing("v8", "lang"));
        } else {
            item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
        }
        item.push(OutElement::leaf("v8", "content", content.clone()));
        c.push(item);
    }
    Ok(c)
}
