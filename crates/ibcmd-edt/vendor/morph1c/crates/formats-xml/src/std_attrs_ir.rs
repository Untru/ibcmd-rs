//! ВАРИАТИВНЫЙ платформенный блок `standardAttributes`/`StandardAttributes` видов с
//! ПРЕДОПРЕДЕЛЁННЫМ набором стандартных атрибутов (child-objects substrate §3.3):
//! `InformationRegister` (Active/LineNumber/Recorder/Period) и `Enum` (Order/Ref).
//!
//! Блок — НЕ пользовательская child-коллекция (нет uuid), а dense property-регион, чей
//! НАБОР атрибутов задан видом, а СОДЕРЖИМОЕ пер-объектно. Виды отличаются ровно
//! декларацией [`IrStdAttrsDecl`] (корневой тег + имена атрибутов + name-определённый
//! `fillChecking`), поэтому оба ведёт ОДНА машинерия (§1.6: канон — в декларации, формат —
//! лишь проекция).
//!
//! ## Что ВАРИАТИВНО (сверено по корпусу, а не по одному источнику)
//! Пер-объектно меняются `synonym`(локализ.), `toolTip`(локализ.), `fillValue`(Value-xsi) и
//! `fullTextSearch`(литерал). Последний РАНЬШЕ был зашит константой `Use` — потому что SSL
//! показывал ровно одно значение (104/104 Enum, 187/187 регистров). Это была ОШИБКА КЛАССА
//! «константа вместо переменной»: платформа даёт свойство ЗАДАВАТЬ, и на чужом корпусе
//! (`integration_subsystem`) witnessed `DontUse` — 2 Enum'а и 6 регистров. Правило: если
//! платформа даёт значение задать, это ДЕФОЛТ, а не инвариант, и он обязан жить в IR.
//! Прочее (`fillChecking`, определённый ИМЕНЕМ: Period=ShowError, иначе DontCheck) —
//! по-прежнему константы и СВЕРЯЮТСЯ строго (§1.0).
//!
//! EDT — РАЗРЕЖЁН (опускает дефолтные `synonym`/`toolTip`/`fillChecking`/`fullTextSearch`);
//! Designer — DENSE (все листья версии всегда).
//!
//! Канонический IR (X by construction): [`PropertyValue::List`] из N элементов (по числу
//! атрибутов вида в фикс-порядке), КАЖДЫЙ — `List([synonym:Localized, toolTip:Localized,
//! fillValue:Value, fullTextSearch:Enum])`. Оба формата → ОДИН список. Пустой `List` = блок
//! опущен целиком (все атрибуты дефолтны). §1.0: любое отклонение от ожидаемой структуры
//! (иной набор/порядок имён, лишний лист, неизвестный fillChecking) → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::locus::StdAttrsDialect;
use crate::value_codec::{self, ValueDialect};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{Lang, PropertyValue, Token, ValueScalarKind, ValueSpec};
use morph1c_core::version::{designer_field_available_in, FormatVersion};

/// Декларация вида: КАКИЕ предопределённые атрибуты он несёт и где живёт Designer-обёртка.
/// Чистые данные — новый вид с предопределённым набором добавляется ЗДЕСЬ, без правок
/// машинерии.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrStdAttrsDecl {
    /// Local-name корневого тега-обёртки Designer (`InformationRegister`/`Enum`): путь —
    /// `<root_elem>/<Properties>/<StandardAttributes>`.
    pub root_elem: &'static str,
    /// Метка вида для диагностики.
    pub label: &'static str,
    /// Предопределённые атрибуты в КАНОНИЧЕСКОМ порядке + их name-определённый
    /// `fillChecking` (`DontCheck` по умолчанию; `Period` — `ShowError`).
    pub attrs: &'static [(&'static str, &'static str)],
}

/// `InformationRegister`: 4 атрибута (сверено 187/187 обе половины).
///
/// УСТАРЕВШАЯ декларация: вид МИГРИРОВАЛ на data-driven
/// `core::spec::metadata::information_register::STD_ATTRS` +
/// [`crate::std_attrs_generic`] — ERP-корпус вскрыл 5 доп. задаваемых листьев
/// (dataHistory/fillChecking/fillFromFillingValue/format/editFormat), которых 4-слотная
/// запись этого кодека не выражает. Коннекторы вида её больше НЕ ссылаются; остаётся
/// здесь до миграции `Enum` (единственного оставшегося пользователя модуля).
pub const INFORMATION_REGISTER: IrStdAttrsDecl = IrStdAttrsDecl {
    root_elem: "InformationRegister",
    label: "InformationRegister",
    attrs: &[
        ("Active", "DontCheck"),
        ("LineNumber", "DontCheck"),
        ("Recorder", "DontCheck"),
        ("Period", "ShowError"),
    ],
};

/// `Enum`: 2 атрибута (сверено 104/104 SSL + 3/3 integration_subsystem).
pub const ENUM: IrStdAttrsDecl = IrStdAttrsDecl {
    root_elem: "Enum",
    label: "Enum",
    attrs: &[("Order", "DontCheck"), ("Ref", "DontCheck")],
};

/// Дефолт `fullTextSearch`: РАЗРЕЖЁННЫЙ EDT опускает лист при этом значении (сверено на
/// Catalog, где `DontUse` witnessed в SSL — тот же сериализатор платформы).
const FTS_DEFAULT: &str = "DontUse";

// ---- canonical IR helpers --------------------------------------------------

fn empty_loc() -> PropertyValue {
    PropertyValue::Localized(Vec::new())
}
fn undefined_value() -> PropertyValue {
    PropertyValue::Value(ValueSpec {
        kind: ValueScalarKind::Undefined,
        scalar: None,
    })
}
fn enum_tok(s: &str) -> PropertyValue {
    PropertyValue::Enum(Token::new(s))
}

/// Одна IR-запись атрибута: `[synonym, toolTip, fillValue, fullTextSearch]`.
#[derive(Debug, Clone)]
struct Record {
    synonym: PropertyValue,
    tooltip: PropertyValue,
    fill_value: PropertyValue,
    full_text: PropertyValue,
}

impl Default for Record {
    fn default() -> Self {
        Record {
            synonym: empty_loc(),
            tooltip: empty_loc(),
            fill_value: undefined_value(),
            full_text: enum_tok(FTS_DEFAULT),
        }
    }
}

impl Record {
    fn pack(self) -> PropertyValue {
        PropertyValue::List(vec![
            self.synonym,
            self.tooltip,
            self.fill_value,
            self.full_text,
        ])
    }
}

/// Канонический IR-маркер блока: `List` из N под-`List`-записей.
fn pack(attrs: Vec<Record>) -> PropertyValue {
    PropertyValue::List(attrs.into_iter().map(Record::pack).collect())
}

/// Распаковать IR-значение в N записей (строго, §1.0).
fn unpack<'a>(
    decl: &IrStdAttrsDecl,
    value: &'a PropertyValue,
) -> Result<Vec<[&'a PropertyValue; 4]>, String> {
    let outer = match value {
        PropertyValue::List(v) => v,
        other => {
            return Err(format!(
                "{} standardAttributes IR must be List, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    };
    if outer.len() != decl.attrs.len() {
        return Err(format!(
            "{} standardAttributes: expected {} attrs, got {}",
            decl.label,
            decl.attrs.len(),
            outer.len()
        ));
    }
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        let inner = match item {
            PropertyValue::List(v) if v.len() == 4 => v,
            _ => {
                return Err(format!(
                    "{} standardAttributes: each attr must be List of 4 (syn,tip,fill,fts)",
                    decl.label
                ))
            }
        };
        out.push([&inner[0], &inner[1], &inner[2], &inner[3]]);
    }
    Ok(out)
}

/// Литерал `fullTextSearch` из IR-значения (строго `Enum`, §1.0).
fn fts_literal<'a>(decl: &IrStdAttrsDecl, v: &'a PropertyValue) -> Result<&'a str, String> {
    match v {
        PropertyValue::Enum(t) => Ok(t.as_str()),
        other => Err(format!(
            "{} standardAttributes fullTextSearch must be Enum, got {:?}",
            decl.label,
            other.kind()
        )),
    }
}

// ---- DECODE + CLAIM --------------------------------------------------------

/// Декодировать блок (от КОРНЯ объекта) в канонический IR-`List`. Строго-тотально.
pub fn decode(
    dialect: StdAttrsDialect,
    decl: &'static IrStdAttrsDecl,
    root: &Element,
    version: FormatVersion,
) -> Decoded {
    let res = match dialect {
        StdAttrsDialect::Edt => decode_edt(decl, root),
        StdAttrsDialect::Designer => decode_designer(decl, root, version),
    };
    match res {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

/// Claim-проход (для leftover): помечает блок РОВНО если он совпал с ожидаемой
/// структурой. Расхождение НЕ клеймит → leftover>0 → §1.0-ошибка корня.
pub fn claim(
    dialect: StdAttrsDialect,
    decl: &'static IrStdAttrsDecl,
    root: &Element,
    version: FormatVersion,
) {
    match dialect {
        StdAttrsDialect::Edt => {
            let _ = decode_edt(decl, root);
        }
        StdAttrsDialect::Designer => {
            let _ = decode_designer(decl, root, version);
        }
    }
}

/// EDT: N подряд `<standardAttributes>`-блоков в каноническом порядке имён. Каждый:
/// `dataHistory=Use`, `name`, [`synonym`], [`toolTip`], `fillValue`, [`fillChecking`],
/// [`fullTextSearch`], `minValue`/`maxValue`=Undefined.
fn decode_edt(decl: &IrStdAttrsDecl, root: &Element) -> Result<PropertyValue, String> {
    let blocks: Vec<&Element> = root
        .children
        .iter()
        .filter(|c| c.local == "standardAttributes" && c.prefix.is_empty())
        .collect();
    if blocks.is_empty() {
        // Блок опущен целиком ⇒ все атрибуты дефолтны ⇒ ПУСТОЙ IR-`List` (симметрично
        // write-омиссии и cf-слоту `{0}`; X/cf-X сходятся на пустом List, R байт-точен).
        return Ok(PropertyValue::List(Vec::new()));
    }
    if blocks.len() != decl.attrs.len() {
        return Err(format!(
            "{} standardAttributes: expected {} <standardAttributes>, found {}",
            decl.label,
            decl.attrs.len(),
            blocks.len()
        ));
    }
    let mut attrs = Vec::with_capacity(decl.attrs.len());
    for (block, (name, fill_checking)) in blocks.iter().zip(decl.attrs) {
        attrs.push(decode_edt_block(decl, block, name, fill_checking)?);
        block.claim();
    }
    Ok(pack(attrs))
}

fn decode_edt_block(
    decl: &IrStdAttrsDecl,
    block: &Element,
    name: &str,
    fill_checking: &str,
) -> Result<Record, String> {
    if !block.attrs.is_empty() {
        return Err(format!(
            "{} standardAttributes block must have no attributes",
            decl.label
        ));
    }
    let mut rec = Record::default();
    // Позиционный проход по ожидаемой последовательности тегов; опциональные —
    // synonym/toolTip/fillChecking/fullTextSearch (EDT опускает их при дефолте).
    let mut it = block.children.iter().peekable();
    expect_text_leaf(decl, &mut it, "dataHistory", "Use")?;
    expect_text_leaf(decl, &mut it, "name", name)?;
    if let Some(c) = it.peek() {
        if c.local == "synonym" && c.prefix.is_empty() {
            rec.synonym = take_localized(it.next().unwrap())?;
        }
    }
    if let Some(c) = it.peek() {
        if c.local == "toolTip" && c.prefix.is_empty() {
            rec.tooltip = take_localized(it.next().unwrap())?;
        }
    }
    // fillValue (Value-xsi, всегда present в EDT-блоке).
    rec.fill_value = {
        let fv = it
            .next()
            .filter(|c| c.local == "fillValue" && c.prefix.is_empty())
            .ok_or_else(|| {
                format!(
                    "{} standardAttributes: expected <fillValue> in block",
                    decl.label
                )
            })?;
        fv.claim();
        value_codec::decode(ValueDialect::Edt, fv)?
    };
    // fillChecking — name-определённая КОНСТАНТА (у Period=ShowError; DontCheck → опущен).
    if fill_checking != "DontCheck" {
        expect_text_leaf(decl, &mut it, "fillChecking", fill_checking)?;
    }
    // fullTextSearch — ВАРИАТИВЕН; EDT опускает его при дефолте `DontUse`.
    if let Some(c) = it.peek() {
        if c.local == "fullTextSearch" && c.prefix.is_empty() {
            let el = it.next().unwrap();
            if !el.attrs.is_empty() || !el.children.is_empty() || el.text.is_empty() {
                return Err(format!(
                    "{} standardAttributes[{name}] <fullTextSearch>: expected a non-empty literal",
                    decl.label
                ));
            }
            el.claim_with_text();
            rec.full_text = enum_tok(&el.text);
        }
    }
    expect_value_undefined(decl, &mut it, "minValue", ValueDialect::Edt)?;
    expect_value_undefined(decl, &mut it, "maxValue", ValueDialect::Edt)?;
    if let Some(extra) = it.next() {
        return Err(format!(
            "{} standardAttributes[{name}]: unexpected extra leaf <{}> (§1.0)",
            decl.label, extra.local
        ));
    }
    Ok(rec)
}

/// Designer-DENSE листья, существующие в версии формата (§1.5): состав плотного региона
/// есть функция версии (2.17 — без `<xr:TypeReductionMode>`, 2.20+ — с ним).
fn designer_leaves(version: FormatVersion) -> Vec<&'static (&'static str, DLeaf)> {
    DESIGNER_LEAVES
        .iter()
        .filter(|(local, _)| designer_field_available_in(local, version))
        .collect()
}

/// Designer: один `<StandardAttributes>` под `<root_elem>/<Properties>` с N
/// `<xr:StandardAttribute name=…>`; каждый — 25 dense `<xr:…>`-листов, из которых
/// ВАРИАТИВНЫ Synonym/ToolTip/FillValue/FullTextSearch, остальные — константы (включая
/// FillChecking, определённый именем).
fn decode_designer(
    decl: &IrStdAttrsDecl,
    root: &Element,
    version: FormatVersion,
) -> Result<PropertyValue, String> {
    let obj = root.child(decl.root_elem).ok_or_else(|| {
        format!(
            "{} StandardAttributes: missing <{}>",
            decl.label, decl.root_elem
        )
    })?;
    let props = obj
        .child("Properties")
        .ok_or_else(|| format!("{} StandardAttributes: missing <Properties>", decl.label))?;
    let wrapper = match props.child("StandardAttributes") {
        Some(w) => w,
        // Обёртка опущена целиком ⇒ все атрибуты дефолтны ⇒ пустой IR-`List`.
        None => return Ok(PropertyValue::List(Vec::new())),
    };
    if !wrapper.prefix.is_empty() || !wrapper.attrs.is_empty() || !wrapper.text.is_empty() {
        return Err(format!(
            "{} <StandardAttributes> must be an unprefixed attribute-less container",
            decl.label
        ));
    }
    if wrapper.children.len() != decl.attrs.len() {
        return Err(format!(
            "{} <StandardAttributes>: expected {} <xr:StandardAttribute>, found {}",
            decl.label,
            decl.attrs.len(),
            wrapper.children.len()
        ));
    }
    let mut attrs = Vec::with_capacity(decl.attrs.len());
    for (attr, (name, fill_checking)) in wrapper.children.iter().zip(decl.attrs) {
        attrs.push(decode_designer_attr(decl, attr, name, fill_checking, version)?);
        attr.claim();
    }
    wrapper.claim();
    Ok(pack(attrs))
}

/// Один из 25 dense-листов Designer: (local, ожидаемая форма). Variable — спец-обработка.
enum DLeaf {
    /// Пустой self-closing `<xr:Tag/>`.
    Empty,
    /// `xsi:nil="true"` self-closing.
    Nil,
    /// Константный текст.
    Text(&'static str),
}

const DESIGNER_LEAVES: &[(&str, DLeaf)] = &[
    ("LinkByType", DLeaf::Empty),
    ("FillChecking", DLeaf::Text("")), // name-определён, спец-проверка
    ("MultiLine", DLeaf::Text("false")),
    ("FillFromFillingValue", DLeaf::Text("false")),
    ("CreateOnInput", DLeaf::Text("Auto")),
    ("TypeReductionMode", DLeaf::Text("TransformValues")),
    ("MaxValue", DLeaf::Nil),
    ("ToolTip", DLeaf::Empty), // variable
    ("ExtendedEdit", DLeaf::Text("false")),
    ("Format", DLeaf::Empty),
    ("ChoiceForm", DLeaf::Empty),
    ("QuickChoice", DLeaf::Text("Auto")),
    ("ChoiceHistoryOnInput", DLeaf::Text("Auto")),
    ("EditFormat", DLeaf::Empty),
    ("PasswordMode", DLeaf::Text("false")),
    ("DataHistory", DLeaf::Text("Use")),
    ("MarkNegatives", DLeaf::Text("false")),
    ("MinValue", DLeaf::Nil),
    ("Synonym", DLeaf::Empty), // variable
    ("Comment", DLeaf::Empty),
    ("FullTextSearch", DLeaf::Text("")), // variable
    ("ChoiceParameterLinks", DLeaf::Empty),
    ("FillValue", DLeaf::Nil), // variable
    ("Mask", DLeaf::Empty),
    ("ChoiceParameters", DLeaf::Empty),
];

fn decode_designer_attr(
    decl: &IrStdAttrsDecl,
    attr: &Element,
    name: &str,
    fill_checking: &str,
    version: FormatVersion,
) -> Result<Record, String> {
    if attr.local != "StandardAttribute" || attr.prefix != "xr" {
        return Err(format!(
            "{} StandardAttributes: expected <xr:StandardAttribute>, got <{}>",
            decl.label, attr.local
        ));
    }
    if attr.attrs.len() != 1 || attr.attrs[0].name != "name" || attr.attrs[0].value != *name {
        return Err(format!(
            "{} <xr:StandardAttribute>: expected single name={name:?}",
            decl.label
        ));
    }
    attr.attrs[0].claimed.set(true);
    let leaves = designer_leaves(version);
    if attr.children.len() != leaves.len() {
        return Err(format!(
            "{} <xr:StandardAttribute name={name:?}>: expected {} leaves, found {}",
            decl.label,
            leaves.len(),
            attr.children.len()
        ));
    }
    let mut rec = Record::default();
    for (child, (local, leaf)) in attr.children.iter().zip(leaves.iter().copied()) {
        if child.local != *local || child.prefix != "xr" {
            return Err(format!(
                "{} <xr:StandardAttribute name={name:?}>: expected <xr:{local}>, got <{}>",
                decl.label, child.local
            ));
        }
        match *local {
            "Synonym" => rec.synonym = take_localized_v8(child)?,
            "ToolTip" => rec.tooltip = take_localized_v8(child)?,
            "FillValue" => {
                child.claim();
                rec.fill_value = value_codec::decode(ValueDialect::Designer, child)?;
            }
            "FullTextSearch" => {
                if !child.attrs.is_empty() || !child.children.is_empty() || child.text.is_empty() {
                    return Err(format!(
                        "{} <xr:FullTextSearch>: expected a non-empty literal, got {:?}",
                        decl.label, child.text
                    ));
                }
                child.claim_with_text();
                rec.full_text = enum_tok(&child.text);
            }
            "FillChecking" => {
                expect_designer_text(decl, child, fill_checking)?;
            }
            _ => verify_designer_const(decl, child, leaf)?,
        }
    }
    Ok(rec)
}

fn verify_designer_const(decl: &IrStdAttrsDecl, child: &Element, leaf: &DLeaf) -> Result<(), String> {
    match leaf {
        DLeaf::Empty => {
            if !child.attrs.is_empty() || !child.children.is_empty() || !child.text.is_empty() {
                return Err(format!(
                    "{} <xr:{}> must be empty self-closing",
                    decl.label, child.local
                ));
            }
            child.claim();
        }
        DLeaf::Nil => {
            if child.children.is_empty()
                && child.text.is_empty()
                && child.attrs.len() == 1
                && child.attrs[0].name == "xsi:nil"
                && child.attrs[0].value == "true"
            {
                child.attrs[0].claimed.set(true);
                child.claim();
            } else {
                return Err(format!(
                    "{} <xr:{}> must be self-closing xsi:nil=\"true\"",
                    decl.label, child.local
                ));
            }
        }
        DLeaf::Text(t) => expect_designer_text(decl, child, t)?,
    }
    Ok(())
}

fn expect_designer_text(decl: &IrStdAttrsDecl, child: &Element, want: &str) -> Result<(), String> {
    if !child.attrs.is_empty() || !child.children.is_empty() || child.text != want {
        return Err(format!(
            "{} <xr:{}>: expected text {want:?}, got {:?}",
            decl.label, child.local, child.text
        ));
    }
    child.claim_with_text();
    Ok(())
}

// ---- EDT leaf parsing helpers ----------------------------------------------

fn expect_text_leaf<'a, I: Iterator<Item = &'a Element>>(
    decl: &IrStdAttrsDecl,
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    want: &str,
) -> Result<(), String> {
    let el = it
        .next()
        .ok_or_else(|| format!("{} standardAttributes: missing <{tag}>", decl.label))?;
    if el.local != tag || !el.prefix.is_empty() {
        return Err(format!(
            "{} standardAttributes: expected <{tag}>, got <{}>",
            decl.label, el.local
        ));
    }
    if !el.attrs.is_empty() || !el.children.is_empty() || el.text != want {
        return Err(format!(
            "{} standardAttributes <{tag}>: expected text {want:?}, got {:?}",
            decl.label, el.text
        ));
    }
    el.claim_with_text();
    Ok(())
}

fn expect_value_undefined<'a, I: Iterator<Item = &'a Element>>(
    decl: &IrStdAttrsDecl,
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    dialect: ValueDialect,
) -> Result<(), String> {
    let el = it
        .next()
        .ok_or_else(|| format!("{} standardAttributes: missing <{tag}>", decl.label))?;
    if el.local != tag || !el.prefix.is_empty() {
        return Err(format!(
            "{} standardAttributes: expected <{tag}>, got <{}>",
            decl.label, el.local
        ));
    }
    el.claim();
    match value_codec::decode(dialect, el)? {
        PropertyValue::Value(ValueSpec {
            kind: ValueScalarKind::Undefined,
            ..
        }) => Ok(()),
        other => Err(format!(
            "{} standardAttributes <{tag}>: expected Undefined, got {:?}",
            decl.label,
            other.kind()
        )),
    }
}

/// EDT localized container `<synonym>/<toolTip>` (`<key>/<value>` pairs). Claim'ит.
fn take_localized(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    let mut it = el.children.iter();
    while let Some(k) = it.next() {
        if k.local != "key" || !k.prefix.is_empty() {
            return Err(format!(
                "stdAttr localized: expected <key>, got <{}>",
                k.local
            ));
        }
        k.claim_with_text();
        let v = it
            .next()
            .ok_or("stdAttr localized: <key> without <value>")?;
        if v.local != "value" || !v.prefix.is_empty() {
            return Err(format!(
                "stdAttr localized: expected <value>, got <{}>",
                v.local
            ));
        }
        v.claim_with_text();
        pairs.push((Lang::new(k.text.clone()), v.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

/// Designer localized container `<xr:Synonym>/<xr:ToolTip>` (`<v8:item>` pairs). Claim'ит.
fn take_localized_v8(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    for item in &el.children {
        if item.local != "item" || item.prefix != "v8" {
            return Err(format!(
                "stdAttr localized v8: expected <v8:item>, got <{}>",
                item.local
            ));
        }
        item.claim();
        let mut it = item.children.iter();
        let lang = it
            .next()
            .filter(|e| e.local == "lang" && e.prefix == "v8")
            .ok_or("v8:item missing v8:lang")?;
        let content = it
            .next()
            .filter(|e| e.local == "content" && e.prefix == "v8")
            .ok_or("v8:item missing v8:content")?;
        if it.next().is_some() {
            return Err("stdAttr localized v8: <v8:item> has extra child".into());
        }
        lang.claim_with_text();
        content.claim_with_text();
        pairs.push((Lang::new(lang.text.clone()), content.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

// ---- ENCODE ----------------------------------------------------------------

/// Восстановить блок byte-exact из IR-`List`. EDT — N узлов `<standardAttributes>`;
/// Designer — 1 узел `<StandardAttributes>` с N `<xr:StandardAttribute>`.
pub fn emit(
    dialect: StdAttrsDialect,
    decl: &'static IrStdAttrsDecl,
    value: &PropertyValue,
    version: FormatVersion,
) -> Result<Vec<OutElement>, String> {
    // Пустой IR-`List` ⇒ блок опущен целиком (все атрибуты дефолтны) — НОЛЬ узлов (byte-exact
    // с source-омиссией; симметрично read-стороне, что отдаёт пустой List при 0 блоков).
    if matches!(value, PropertyValue::List(v) if v.is_empty()) {
        return Ok(Vec::new());
    }
    let attrs = unpack(decl, value)?;
    match dialect {
        StdAttrsDialect::Edt => {
            let mut out = Vec::with_capacity(decl.attrs.len());
            for ((name, fill_checking), rec) in decl.attrs.iter().zip(&attrs) {
                out.push(emit_edt_block(decl, name, fill_checking, rec)?);
            }
            Ok(out)
        }
        StdAttrsDialect::Designer => {
            let mut wrapper = OutElement::branch("", "StandardAttributes");
            for ((name, fill_checking), rec) in decl.attrs.iter().zip(&attrs) {
                wrapper.push(emit_designer_attr(decl, name, fill_checking, rec, version)?);
            }
            Ok(vec![wrapper])
        }
    }
}

fn emit_edt_block(
    decl: &IrStdAttrsDecl,
    name: &str,
    fill_checking: &str,
    rec: &[&PropertyValue; 4],
) -> Result<OutElement, String> {
    let mut block = OutElement::branch("", "standardAttributes");
    block.push(OutElement::leaf("", "dataHistory", "Use"));
    block.push(OutElement::leaf("", "name", name.to_string()));
    if let Some(loc) = emit_localized_keyval(decl, "synonym", rec[0])? {
        block.push(loc);
    }
    if let Some(loc) = emit_localized_keyval(decl, "toolTip", rec[1])? {
        block.push(loc);
    }
    block.push(value_codec::encode(
        ValueDialect::Edt,
        "",
        "fillValue",
        as_value(decl, rec[2])?,
    )?);
    if fill_checking != "DontCheck" {
        block.push(OutElement::leaf(
            "",
            "fillChecking",
            fill_checking.to_string(),
        ));
    }
    // fullTextSearch — РАЗРЕЖЁН: опускается при дефолте (`DontUse`).
    let fts = fts_literal(decl, rec[3])?;
    if fts != FTS_DEFAULT {
        block.push(OutElement::leaf("", "fullTextSearch", fts.to_string()));
    }
    block.push(value_codec::encode(
        ValueDialect::Edt,
        "",
        "minValue",
        &undefined_spec(),
    )?);
    block.push(value_codec::encode(
        ValueDialect::Edt,
        "",
        "maxValue",
        &undefined_spec(),
    )?);
    Ok(block)
}

fn emit_designer_attr(
    decl: &IrStdAttrsDecl,
    name: &str,
    fill_checking: &str,
    rec: &[&PropertyValue; 4],
    version: FormatVersion,
) -> Result<OutElement, String> {
    let mut attr = OutElement::branch("xr", "StandardAttribute").attr("name", name);
    for (local, leaf) in designer_leaves(version) {
        let el = match *local {
            "Synonym" => emit_localized_v8(decl, "Synonym", rec[0])?,
            "ToolTip" => emit_localized_v8(decl, "ToolTip", rec[1])?,
            "FillValue" => value_codec::encode(
                ValueDialect::Designer,
                "xr",
                "FillValue",
                as_value(decl, rec[2])?,
            )?,
            "FullTextSearch" => OutElement::leaf(
                "xr",
                "FullTextSearch",
                fts_literal(decl, rec[3])?.to_string(),
            ),
            "FillChecking" => OutElement::leaf("xr", "FillChecking", fill_checking.to_string()),
            _ => match leaf {
                DLeaf::Empty => OutElement::self_closing("xr", *local),
                DLeaf::Nil => OutElement::self_closing("xr", *local).attr("xsi:nil", "true"),
                DLeaf::Text(t) => OutElement::leaf("xr", *local, (*t).to_string()),
            },
        };
        attr.push(el);
    }
    Ok(attr)
}

fn undefined_spec() -> ValueSpec {
    ValueSpec {
        kind: ValueScalarKind::Undefined,
        scalar: None,
    }
}

fn as_value<'a>(decl: &IrStdAttrsDecl, v: &'a PropertyValue) -> Result<&'a ValueSpec, String> {
    match v {
        PropertyValue::Value(s) => Ok(s),
        other => Err(format!(
            "{} standardAttributes fillValue must be Value, got {:?}",
            decl.label,
            other.kind()
        )),
    }
}

/// EDT localized: пустой → опустить (`None`); непустой → `<tag><key>…<value>…</tag>`.
fn emit_localized_keyval(
    decl: &IrStdAttrsDecl,
    tag: &str,
    v: &PropertyValue,
) -> Result<Option<OutElement>, String> {
    let pairs = as_localized(decl, v)?;
    if pairs.is_empty() {
        return Ok(None);
    }
    let mut c = OutElement::branch("", tag);
    for (lang, text) in pairs {
        c.push(OutElement::leaf("", "key", lang.as_str().to_string()));
        c.push(OutElement::leaf("", "value", text.clone()));
    }
    Ok(Some(c))
}

/// Designer localized: пустой → `<xr:Tag/>`; непустой → `<xr:Tag><v8:item>…`.
fn emit_localized_v8(
    decl: &IrStdAttrsDecl,
    tag: &str,
    v: &PropertyValue,
) -> Result<OutElement, String> {
    let pairs = as_localized(decl, v)?;
    if pairs.is_empty() {
        return Ok(OutElement::self_closing("xr", tag));
    }
    let mut c = OutElement::branch("xr", tag);
    for (lang, content) in pairs {
        let mut item = OutElement::branch("v8", "item");
        item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
        item.push(OutElement::leaf("v8", "content", content.clone()));
        c.push(item);
    }
    Ok(c)
}

fn as_localized<'a>(
    decl: &IrStdAttrsDecl,
    v: &'a PropertyValue,
) -> Result<&'a Vec<(Lang, String)>, String> {
    match v {
        PropertyValue::Localized(p) => Ok(p),
        other => Err(format!(
            "{} standardAttributes synonym/toolTip must be Localized, got {:?}",
            decl.label,
            other.kind()
        )),
    }
}
