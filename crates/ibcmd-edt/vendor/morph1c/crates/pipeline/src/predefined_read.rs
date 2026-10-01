//! Чтение/запись ПРЕДОПРЕДЕЛЁННЫХ ДАННЫХ в DESIGNER-диалекте — сайдкар
//! `<dir>/<Name>/Ext/Predefined.xml` (§1.2/§4). Сиблинг [`crate::exchange_plan_content_read`].
//!
//! # Почему сайдкар нужен ОТДЕЛЬНЫМ проходом
//! EDT несёт предопределённые ПРЯМО в дескрипторе (`<predefined><items…>` — кодеки
//! `formats_xml::predefined` / `::predefined_cct` в спек-поле `predefined`). Designer в
//! дескрипторе `Catalogs/<Name>.xml` их НЕ несёт ВООБЩЕ — они лежат отдельным
//! `Ext/Predefined.xml` (как формы/модули). Кодек `locus::PredefinedData` до этого прохода
//! документировался «EDT-only, Designer тело не несёт» — ЭТО БЫЛО НЕВЕРНО (15 сайдкаров SSL).
//!
//! Цена ошибки: cf-ассемблер регистрирует пути предопределённых
//! (`formats_cf::predefined_body::harvest_predefined_paths` внутри
//! `build_ir_object_path_registry`), поэтому designer-IR без этого блока НЕ РЕЗОЛВИЛ ссылки
//! ВИДА `Catalog.ВидыКонтактнойИнформации.СправочникПользователи` → `designer→cf` отказывал
//! на 2 объектах. Read-сторона — единственное, что требовалось: write-сторона cf не меняется.
//!
//! # Раскладка (RE — 15/15 сайдкаров SSL: 13 Catalog + 2 ChartOfCharacteristicTypes; плюс
//! ERP-корпус CCT — 8 сайдкаров с кодами/папками, см. ниже)
//! С BOM, CRLF, ТАБ-отступ, БЕЗ хвостового перевода строки (конвенция всех Designer-сайдкаров).
//! Корень `<PredefinedData …>` (ns `http://v8.1c.ru/8.3/xcf/predef`) с
//! `xsi:type="CatalogPredefinedItems"` (Catalog) / `"PlanOfCharacteristicKindPredefinedItems"`
//! (CCT) / `"ChartOfAccountsPredefinedItems"` (CoA) и `version="2.20"|"2.21"` — версия
//! ФАЙЛА-источника (детект через [`crate::sidecar_version`], НЕ хардкод: ERP-сайдкары
//! несут `2.20`).
//!
//! Узел — `<Item id="uuid">`, DENSE (в отличие от SPARSE EDT):
//! * Catalog: `<Name>`, `<Code/>`|`<Code>текст</Code>`|`<Code xsi:type="xs:decimal">цифры`
//!   (числовые коды каталогов с codeType=Number — ERP-перепись 130 сайдкаров: 3292 пустых +
//!   1864 текстовых БЕЗ xsi:type + 289 `xs:decimal`, все непустые цифры; зеркалит EDT
//!   `<code xsi:type="core:NumberValue">`), `<Description/>`|`<Description>…`,
//!   `<IsFolder>true|false</IsFolder>`, `[<ChildItems><Item…></ChildItems>]` (РЕКУРСИВНО).
//! * CCT: `<Name>`, `<Code/>`|`<Code>текст</Code>`, `<Description>`, `<Type>…</Type>`
//!   (у ПАПОК — пустой `<Type/>`), `<IsFolder>true|false</IsFolder>`,
//!   `[<ChildItems>…]` (РЕКУРСИВНО — та же вложенность, что у Catalog). Прежняя редакция
//!   запекала `<Code/>`/`IsFolder=false` константами (witnessed-only SSL 2/2) — ERP её
//!   опроверг: `АналитикиСтатейБюджетов` несёт коды (`000000016`, …),
//!   `СтатьиАктивовПассивов`/`мирПроизвольныеПараметры` — папки с `<ChildItems>` до
//!   3 уровней. Канонический CCT-узел теперь моделирует code/isFolder/children
//!   (`formats_xml::predefined_cct`, N=10, префикс [0..7] == Catalog-узлу).
//! * CoA (`ChartOfAccounts`, ERP-witnessed Хозрасчетный 438 + Международный 1): `<Name>`,
//!   `<Code>`, `<Description>`, `<AccountType>Active|Passive|ActivePassive`,
//!   `<OffBalance>bool`, `<Order>` (ведущие пробелы значимы), `<AccountingFlags>` с
//!   `<Flag ref="путь">bool</Flag>` — DENSE все объявленные флаги,
//!   `<ExtDimensionTypes>`(`/`) с `<ExtDimensionType name="путь-вида-субконто">`:
//!   `<Turnover>bool` + DENSE `<AccountingFlags>` ед-флагов, `[<ChildItems>…]`
//!   (РЕКУРСИВНО, до 3 уровней). Канон — `formats_xml::predefined_coa` (несёт лишь
//!   ИСТИННЫЕ флаги; DENSE-набор восстанавливают декларации объекта [`CoaDecl`]).
//!
//! Канон — ТОТ ЖЕ IR, что даёт EDT-read (§3.5, X by construction): presence-флаги
//! `has_desc`/`has_folder`/`has_code` восстанавливают EDT-РАЗРЕЖЕННОСТЬ (EDT опускает
//! пустой `<description>`, `<isFolder>false</isFolder>` и пустой `<code>`), поэтому
//! `has_desc = !desc.is_empty()`, `has_folder = is_folder`, `has_code = !code.is_empty()`.
//! Designer-emit флаги игнорирует (пишет DENSE всегда).
//!
//! Авто-префикс ссылочного ns в `<v8:Type>` — `d{N}p1`, где N = 1-based глубина host'а
//! `<v8:Type>`: 4 у корневого `<Item>`, +2 на каждый уровень `<ChildItems>` (witnessed ERP:
//! d4p1/d6p1/d8p1/d10p1 — `мирПроизвольныеПараметры` 94/219/3/0,
//! `СтатьиАктивовПассивов` 0/108/39/2).
//!
//! §1.0-самопроверка на read: пере-сериализация канона (версией ИСТОЧНИКА) обязана
//! воспроизвести исходные байты; write штампует версию амбьентного round-trip-таргета
//! ([`crate::sidecar_version::write_target`]).

use std::path::{Path, PathBuf};

use formats_xml::emit::{render, Envelope, OutElement};
use formats_xml::registry::Format;
use formats_xml::type_codec::{self, TypeDialect};
use morph1c_core::ir::value::{PropertyValue, Token, TypeSpec};
use morph1c_core::ir::{FieldId, MetadataObject};
use morph1c_core::spec::metadata::catalog::F_PREDEFINED as CAT_PREDEFINED;
use morph1c_core::spec::metadata::chart_of_accounts::F_PREDEFINED as COA_PREDEFINED;
use morph1c_core::spec::metadata::chart_of_characteristic_types::F_PREDEFINED as CCT_PREDEFINED;

use crate::ConvertError;

/// Имя Designer-сайдкара внутри `Ext/`.
const FILE: &str = "Predefined.xml";

/// Envelope Designer-сайдкара (та же байтовая обёртка, что у дескриптора: BOM, CRLF, таб,
/// без хвостового EOL; `>` экранируется, `"` — нет).
const ENVELOPE: Envelope = Envelope {
    bom: true,
    eol: "\r\n",
    indent_unit: "\t",
    decl: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
    trailing_eol: false,
    escape_gt: true,
    escape_quot: false,
    text_eol: "\n",
};

/// Глубина host-элемента `<v8:Type>` КОРНЕВОГО `<Item>` сайдкара
/// (`PredefinedData`>`Item`>`Type`>`v8:Type`) — 1С нумерует АВТО-префикс ссылочного ns
/// позицией (`d4p1`). Каждый уровень `<ChildItems>` добавляет 2 (`d6p1`/`d8p1`/`d10p1` —
/// witnessed ERP `мирПроизвольныеПараметры`/`СтатьиАктивовПассивов`). См.
/// [`type_codec::encode_scoped_auto_cfg`].
const TYPE_HOST_DEPTH: usize = 4;

/// Глубина host'а `<v8:Type>` для уровня вложенности `level` (0 = корневой `<Item>`).
const fn type_host_depth(level: usize) -> usize {
    TYPE_HOST_DEPTH + 2 * level
}

/// Вид-носитель предопределённых: `(kind, spec-поле, xsi:type корня)`.
const CARRIERS: &[(&str, FieldId, &str)] = &[
    ("Catalog", CAT_PREDEFINED, "CatalogPredefinedItems"),
    (
        "ChartOfCharacteristicTypes",
        CCT_PREDEFINED,
        "PlanOfCharacteristicKindPredefinedItems",
    ),
    // CoA — СВОЙ Item-шейп (см. [`decode_coa_item`]; ERP-witnessed 439 узлов обоих планов).
    ("ChartOfAccounts", COA_PREDEFINED, "ChartOfAccountsPredefinedItems"),
];

/// Найти вид-носитель по имени вида.
fn carrier(kind: &str) -> Option<(FieldId, &'static str)> {
    CARRIERS
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, f, x)| (*f, *x))
}

/// Является ли вид CCT-вариантом узла (`<Type>` вместо `<ChildItems>`).
fn is_cct(kind: &str) -> bool {
    kind == "ChartOfCharacteristicTypes"
}

// --- индексы канонических узлов (форма кодеков `formats_xml::predefined[_cct]`) ------------

/// Catalog: `List([id, name, has_desc, desc, code, has_folder, is_folder, children,
/// code_is_number])` — индексы РЕЭКСПОРТИРОВАНЫ из канона `formats_xml::predefined`
/// (единый источник; `code_is_number` добавлен ERP-раундом — числовые коды).
mod cat_node {
    pub use formats_xml::predefined::{
        I_CHILDREN as CHILDREN, I_CODE as CODE, I_CODE_IS_NUMBER as CODE_IS_NUMBER,
        I_DESC as DESC, I_FOLDER as IS_FOLDER, I_HAS_DESC as HAS_DESC,
        I_HAS_FOLDER as HAS_FOLDER, I_ID as ID, I_NAME as NAME, N,
    };
}
/// CCT: `List([id, name, has_desc, desc, code, has_folder, is_folder, children, has_code,
/// type])` — индексы РЕЭКСПОРТИРОВАНЫ из канона `formats_xml::predefined_cct` (единый
/// источник; префикс [0..7] совпадает с Catalog-узлом).
mod cct_node {
    pub use formats_xml::predefined_cct::{
        I_CHILDREN as CHILDREN, I_CODE as CODE, I_DESC as DESC, I_FOLDER as IS_FOLDER,
        I_HAS_CODE as HAS_CODE, I_HAS_DESC as HAS_DESC, I_HAS_FOLDER as HAS_FOLDER, I_ID as ID,
        I_NAME as NAME, I_TYPE as TYPE, N,
    };
}
/// CoA: `List([id, name, has_desc, desc, code, account_type, off_balance, order,
/// accounting_flags, ext_dimensions, children])` — индексы РЕЭКСПОРТИРОВАНЫ из канона
/// `formats_xml::predefined_coa` (ED-узел: `List([char_type, turnover, flags])`).
mod coa_node {
    pub use formats_xml::predefined_coa::{
        ED_CHAR_TYPE, ED_FLAGS, ED_N, ED_TURNOVER, I_ACCOUNTING_FLAGS as ACCOUNTING_FLAGS,
        I_ACCOUNT_TYPE as ACCOUNT_TYPE, I_CHILDREN as CHILDREN, I_CODE as CODE, I_DESC as DESC,
        I_EXT_DIMENSIONS as EXT_DIMENSIONS, I_HAS_DESC as HAS_DESC, I_ID as ID, I_NAME as NAME,
        I_OFF_BALANCE as OFF_BALANCE, I_ORDER as ORDER, N,
    };
}

/// Декларации признаков учёта плана счетов (ПОЛНЫЕ ref-пути в порядке объявления) —
/// контекст DENSE-эмиссии Designer-сайдкара CoA: канон несёт лишь ИСТИННЫЕ флаги
/// (EDT-разрежённость), Designer пишет ВСЕ объявленные с true/false. Строится из
/// child-коллекций IR-объекта ([`coa_decl_of`]).
struct CoaDecl {
    /// `ChartOfAccounts.<Имя>.AccountingFlag.<Флаг>` в порядке объявления.
    flags: Vec<String>,
    /// `ChartOfAccounts.<Имя>.ExtDimensionAccountingFlag.<Флаг>` в порядке объявления.
    ed_flags: Vec<String>,
}

/// Собрать [`CoaDecl`] из child-коллекций IR-объекта (порядок детей = порядок объявления).
fn coa_decl_of(obj: &MetadataObject) -> CoaDecl {
    let paths = |child_kind: &str, seg: &str| -> Vec<String> {
        obj.children
            .iter()
            .filter(|c| c.kind.as_str() == child_kind)
            .map(|c| format!("ChartOfAccounts.{}.{seg}.{}", obj.name, c.name))
            .collect()
    };
    CoaDecl {
        flags: paths("ChartOfAccounts.AccountingFlag", "AccountingFlag"),
        ed_flags: paths(
            "ChartOfAccounts.ExtDimensionAccountingFlag",
            "ExtDimensionAccountingFlag",
        ),
    }
}

// --- READ ---------------------------------------------------------------------------------

/// Подгрузить предопределённые из Designer-сайдкара в спек-поле `predefined`. EDT (блок в
/// дескрипторе) и cf (контейнер) — no-op; не-носитель — no-op; отсутствие файла — no-op
/// (у вида без предопределённых сайдкара просто нет).
pub fn attach_predefined(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(());
    }
    let Some((field, xsi_type)) = carrier(kind) else {
        return Ok(());
    };
    let Some(path) = sidecar_path(descriptor_path) else {
        return Ok(());
    };
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let read_err = |reason: String| ConvertError::Read {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason,
    };

    let doc = formats_xml::parse(&bytes)
        .map_err(|e| read_err(format!("{FILE} {}: {e}", path.display())))?;
    let root = &doc.root;
    if root.local != "PredefinedData" {
        return Err(read_err(format!(
            "{FILE} root is <{}>, expected <PredefinedData> (§1.0)",
            root.local
        )));
    }
    match root.attr("xsi:type") {
        Some(a) if a.value == xsi_type => {}
        Some(a) => {
            return Err(read_err(format!(
                "{FILE} xsi:type={:?}, expected {xsi_type:?} for kind {kind} (§1.0)",
                a.value
            )))
        }
        None => return Err(read_err(format!("{FILE} root has no xsi:type (§1.0)"))),
    }
    // Версия ФАЙЛА-источника (FORMATS.md §1: формат входа детектится): witnessed-реестр
    // через sidecar_version (ERP-сайдкары несут `2.20`, SSL — `2.21`; §1.0-самопроверка
    // ниже пере-сериализует ИМЕННО ею — иначе ERP-источник ложно падал бы на version=).
    let source_version = match root.attr("version") {
        Some(a) => {
            crate::sidecar_version::parse_witnessed(&a.value, FILE).map_err(&read_err)?;
            a.value.clone()
        }
        None => return Err(read_err(format!("{FILE} root has no version attribute (§1.0)"))),
    };

    let is_coa = kind == "ChartOfAccounts";
    let mut items = Vec::with_capacity(root.children.len());
    for child in &root.children {
        let item = if is_coa {
            decode_coa_item(child).map_err(read_err)?
        } else {
            decode_item(kind, child).map_err(read_err)?
        };
        items.push(item);
    }
    if items.is_empty() {
        return Err(read_err(format!(
            "{FILE} declares no <Item> (§1.0 — an empty sidecar is unwitnessed; a kind without \
             predefined data simply has no file)"
        )));
    }
    // §1.0: дескриптор-read блок НЕ заполняет (Designer его там не несёт) — только этот проход.
    if obj.get(field).is_some() {
        return Err(read_err(
            "object already carries a predefined block before the sidecar attach (unexpected — \
             the Designer descriptor does not carry it)"
                .into(),
        ));
    }
    let predefined = PropertyValue::List(items);
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (версией
    // источника — write-путь штампует версию таргета отдельно). CoA-эмиссия DENSE
    // требует деклараций флагов объекта ([`CoaDecl`]) — несоответствие источника
    // декларациям (иной набор/порядок Flag'ов) провалит byte-сверку → типизированный отказ.
    let coa_decl = is_coa.then(|| coa_decl_of(obj));
    let re = serialize(kind, xsi_type, &predefined, &source_version, coa_decl.as_ref())
        .map_err(read_err)?;
    if re != bytes {
        return Err(read_err(format!(
            "{FILE} {} does not round-trip byte-exactly through the IR (§1.0)",
            path.display()
        )));
    }
    obj.properties.push((field, predefined));
    Ok(())
}

/// Разобрать `<Item id="uuid">` (рекурсивно по `<ChildItems>` — Catalog SSL 15/15 и CCT
/// ERP `СтатьиАктивовПассивов`/`мирПроизвольныеПараметры`).
fn decode_item(kind: &str, el: &formats_xml::descriptor::Element) -> Result<PropertyValue, String> {
    if el.local != "Item" {
        return Err(format!(
            "{FILE} carries <{}>, only <Item> witnessed (§1.0)",
            el.local
        ));
    }
    let id = el
        .attr("id")
        .ok_or_else(|| format!("{FILE} <Item> has no @id (§1.0)"))?
        .value
        .clone();

    let mut name: Option<String> = None;
    let mut code: Option<String> = None;
    let mut code_is_number = false;
    let mut desc: Option<String> = None;
    let mut type_spec: Option<TypeSpec> = None;
    let mut is_folder: Option<bool> = None;
    let mut children: Vec<PropertyValue> = Vec::new();

    for c in &el.children {
        match c.local.as_str() {
            "Name" => name = Some(c.text.clone()),
            "Code" => {
                // Числовой код — `<Code xsi:type="xs:decimal">цифры</Code>` (ERP-witnessed
                // 289/289 непустые цифры; только Catalog — CCT-коды всегда плоский текст).
                code_is_number = match c.attr("xsi:type") {
                    Some(a) if a.value == "xs:decimal" && !is_cct(kind) => {
                        if c.text.is_empty() || !c.text.bytes().all(|b| b.is_ascii_digit()) {
                            return Err(format!(
                                "{FILE} <Code xsi:type=\"xs:decimal\"> must be non-empty \
                                 digits (witnessed 289/289), got {:?} (§1.0)",
                                c.text
                            ));
                        }
                        true
                    }
                    Some(a) => {
                        return Err(format!(
                            "{FILE} <Code> carries unwitnessed xsi:type={:?} for kind {kind} \
                             (§1.0)",
                            a.value
                        ))
                    }
                    None => false,
                };
                code = Some(c.text.clone());
            }
            "Description" => desc = Some(c.text.clone()),
            "IsFolder" => {
                is_folder = Some(match c.text.as_str() {
                    "true" => true,
                    "false" => false,
                    o => return Err(format!("{FILE} <IsFolder> bool expected, got {o:?} (§1.0)")),
                })
            }
            "Type" if is_cct(kind) => {
                type_spec = Some(match type_codec::decode(TypeDialect::Designer, c)? {
                    PropertyValue::Type(t) => t,
                    other => {
                        return Err(format!(
                            "{FILE} <Type> decoded to {:?}, want Type (§1.0)",
                            other.kind()
                        ))
                    }
                })
            }
            // Рекурсия обоих видов: Catalog (SSL 15/15) и CCT (ERP `СтатьиАктивовПассивов`/
            // `мирПроизвольныеПараметры` — папки с вложенностью до 3 уровней).
            "ChildItems" => {
                for gc in &c.children {
                    children.push(decode_item(kind, gc)?);
                }
                if children.is_empty() {
                    return Err(format!(
                        "{FILE} <ChildItems> is empty (§1.0 — unwitnessed; an item without \
                         children simply has no <ChildItems>)"
                    ));
                }
            }
            other => {
                return Err(format!(
                    "{FILE} <Item> carries unexpected <{other}> for kind {kind} (§1.0)"
                ))
            }
        }
    }

    let name = name.ok_or_else(|| format!("{FILE} <Item> has no <Name> (§1.0)"))?;
    let code = code.ok_or_else(|| format!("{FILE} <Item> has no <Code> (§1.0)"))?;
    let desc = desc.ok_or_else(|| format!("{FILE} <Item> has no <Description> (§1.0)"))?;
    let is_folder = is_folder.ok_or_else(|| format!("{FILE} <Item> has no <IsFolder> (§1.0)"))?;

    // Presence-флаги = EDT-РАЗРЕЖЕННОСТЬ (EDT опускает пустой `<description>`,
    // `<isFolder>false</isFolder>` и пустой `<code>`) → тот же канон, что даёт EDT-read (§3.5).
    let has_desc = !desc.is_empty();

    if is_cct(kind) {
        // CCT-узел (канон `formats_xml::predefined_cct`, N=10): code/isFolder/children
        // моделируются с ERP-раунда (witness `АналитикиСтатейБюджетов` — 41 код;
        // `СтатьиАктивовПассивов` — папки/рекурсия); прежний §1.0-отказ «non-empty <Code>
        // unwitnessed» снят вместе с расширением канона. `<Type/>` папок декодится в
        // ПУСТОЙ TypeSpec (witnessed: папка ⟺ пустой Type).
        let type_spec =
            type_spec.ok_or_else(|| format!("{FILE} CCT <Item> has no <Type> (§1.0)"))?;
        let mut node = vec![PropertyValue::Bool(false); cct_node::N];
        node[cct_node::ID] = PropertyValue::Str(id);
        node[cct_node::NAME] = PropertyValue::Str(name);
        node[cct_node::HAS_DESC] = PropertyValue::Bool(has_desc);
        node[cct_node::DESC] = PropertyValue::Str(desc);
        node[cct_node::HAS_CODE] = PropertyValue::Bool(!code.is_empty());
        node[cct_node::CODE] = PropertyValue::Str(code);
        node[cct_node::HAS_FOLDER] = PropertyValue::Bool(is_folder);
        node[cct_node::IS_FOLDER] = PropertyValue::Bool(is_folder);
        node[cct_node::CHILDREN] = PropertyValue::List(children);
        node[cct_node::TYPE] = PropertyValue::Type(type_spec);
        return Ok(PropertyValue::List(node));
    }

    let mut node = vec![PropertyValue::Bool(false); cat_node::N];
    node[cat_node::ID] = PropertyValue::Str(id);
    node[cat_node::NAME] = PropertyValue::Str(name);
    node[cat_node::HAS_DESC] = PropertyValue::Bool(has_desc);
    node[cat_node::DESC] = PropertyValue::Str(desc);
    node[cat_node::CODE] = PropertyValue::Str(code);
    node[cat_node::HAS_FOLDER] = PropertyValue::Bool(is_folder);
    node[cat_node::IS_FOLDER] = PropertyValue::Bool(is_folder);
    node[cat_node::CHILDREN] = PropertyValue::List(children);
    node[cat_node::CODE_IS_NUMBER] = PropertyValue::Bool(code_is_number);
    Ok(PropertyValue::List(node))
}

// --- CoA (свой Item-шейп; ERP-witnessed 439 узлов) -----------------------------------------

/// Разобрать `<Item id>` плана счетов (DENSE): `Name, Code, Description, AccountType,
/// OffBalance, Order, AccountingFlags{<Flag ref>bool — ВСЕ объявленные флаги},
/// ExtDimensionTypes{<ExtDimensionType name>: Turnover + AccountingFlags}[, ChildItems]`.
/// Канон — тот же, что даёт EDT-кодек `formats_xml::predefined_coa` (§3.5): несёт лишь
/// ИСТИННЫЕ флаги (EDT-разрежённость); DENSE-набор восстанавливается декларациями
/// объекта на эмиссии ([`CoaDecl`]).
fn decode_coa_item(el: &formats_xml::descriptor::Element) -> Result<PropertyValue, String> {
    if el.local != "Item" {
        return Err(format!(
            "{FILE} carries <{}>, only <Item> witnessed (§1.0)",
            el.local
        ));
    }
    let id = el
        .attr("id")
        .ok_or_else(|| format!("{FILE} <Item> has no @id (§1.0)"))?
        .value
        .clone();

    let mut name: Option<String> = None;
    let mut code: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut account_type: Option<String> = None;
    let mut off_balance: Option<bool> = None;
    let mut order: Option<String> = None;
    let mut flags: Vec<PropertyValue> = Vec::new();
    let mut eds: Vec<PropertyValue> = Vec::new();
    let mut children: Vec<PropertyValue> = Vec::new();

    let parse_bool = |t: &str, what: &str| -> Result<bool, String> {
        match t {
            "true" => Ok(true),
            "false" => Ok(false),
            o => Err(format!("{FILE} <{what}> bool expected, got {o:?} (§1.0)")),
        }
    };
    // `<Flag ref="путь">bool</Flag>`-контейнер → ref-пути ИСТИННЫХ флагов (по порядку).
    let decode_flags = |host: &formats_xml::descriptor::Element| -> Result<Vec<PropertyValue>, String> {
        let mut out = Vec::new();
        for f in &host.children {
            if f.local != "Flag" {
                return Err(format!(
                    "{FILE} <AccountingFlags> carries <{}>, only <Flag> witnessed (§1.0)",
                    f.local
                ));
            }
            let path = f
                .attr("ref")
                .ok_or_else(|| format!("{FILE} <Flag> has no @ref (§1.0)"))?
                .value
                .clone();
            if parse_bool(&f.text, "Flag")? {
                out.push(PropertyValue::Str(path));
            }
        }
        Ok(out)
    };

    for c in &el.children {
        match c.local.as_str() {
            "Name" => name = Some(c.text.clone()),
            "Code" => code = Some(c.text.clone()),
            "Description" => desc = Some(c.text.clone()),
            "AccountType" => {
                let t = c.text.clone();
                if !["Active", "Passive", "ActivePassive"].contains(&t.as_str()) {
                    return Err(format!(
                        "{FILE} <AccountType> literal must be Active/Passive/ActivePassive, \
                         got {t:?} (§1.0)"
                    ));
                }
                account_type = Some(t);
            }
            "OffBalance" => off_balance = Some(parse_bool(&c.text, "OffBalance")?),
            "Order" => order = Some(c.text.clone()),
            "AccountingFlags" => flags = decode_flags(c)?,
            "ExtDimensionTypes" => {
                for e in &c.children {
                    if e.local != "ExtDimensionType" {
                        return Err(format!(
                            "{FILE} <ExtDimensionTypes> carries <{}>, only <ExtDimensionType> \
                             witnessed (§1.0)",
                            e.local
                        ));
                    }
                    let ct = e
                        .attr("name")
                        .ok_or_else(|| format!("{FILE} <ExtDimensionType> has no @name (§1.0)"))?
                        .value
                        .clone();
                    let mut turnover = false;
                    let mut ed_flags: Vec<PropertyValue> = Vec::new();
                    for x in &e.children {
                        match x.local.as_str() {
                            "Turnover" => turnover = parse_bool(&x.text, "Turnover")?,
                            "AccountingFlags" => ed_flags = decode_flags(x)?,
                            other => {
                                return Err(format!(
                                    "{FILE} <ExtDimensionType> carries unexpected <{other}> (§1.0)"
                                ))
                            }
                        }
                    }
                    eds.push(PropertyValue::List(vec![
                        PropertyValue::Str(ct),
                        PropertyValue::Bool(turnover),
                        PropertyValue::List(ed_flags),
                    ]));
                }
            }
            "ChildItems" => {
                for gc in &c.children {
                    children.push(decode_coa_item(gc)?);
                }
                if children.is_empty() {
                    return Err(format!(
                        "{FILE} <ChildItems> is empty (§1.0 — unwitnessed; an item without \
                         children simply has no <ChildItems>)"
                    ));
                }
            }
            other => {
                return Err(format!(
                    "{FILE} <Item> carries unexpected <{other}> for kind ChartOfAccounts (§1.0)"
                ))
            }
        }
    }

    let name = name.ok_or_else(|| format!("{FILE} <Item> has no <Name> (§1.0)"))?;
    let code = code.ok_or_else(|| format!("{FILE} <Item> has no <Code> (§1.0)"))?;
    let desc = desc.ok_or_else(|| format!("{FILE} <Item> has no <Description> (§1.0)"))?;
    let account_type =
        account_type.ok_or_else(|| format!("{FILE} <Item> has no <AccountType> (§1.0)"))?;
    let off_balance =
        off_balance.ok_or_else(|| format!("{FILE} <Item> has no <OffBalance> (§1.0)"))?;
    let order = order.ok_or_else(|| format!("{FILE} <Item> has no <Order> (§1.0)"))?;

    let mut node = vec![PropertyValue::Bool(false); coa_node::N];
    node[coa_node::ID] = PropertyValue::Str(id);
    node[coa_node::NAME] = PropertyValue::Str(name);
    // has_desc = EDT-разрежённость (EDT опускает пустой <description> — как Catalog/CCT).
    node[coa_node::HAS_DESC] = PropertyValue::Bool(!desc.is_empty());
    node[coa_node::DESC] = PropertyValue::Str(desc);
    node[coa_node::CODE] = PropertyValue::Str(code);
    node[coa_node::ACCOUNT_TYPE] = PropertyValue::Enum(Token::new(account_type));
    node[coa_node::OFF_BALANCE] = PropertyValue::Bool(off_balance);
    node[coa_node::ORDER] = PropertyValue::Str(order);
    node[coa_node::ACCOUNTING_FLAGS] = PropertyValue::List(flags);
    node[coa_node::EXT_DIMENSIONS] = PropertyValue::List(eds);
    node[coa_node::CHILDREN] = PropertyValue::List(children);
    Ok(PropertyValue::List(node))
}

/// Эмитировать CoA `<Item>` (DENSE): все объявленные флаги с true/false (истинность —
/// членство пути в каноне), `<AccountingFlags/>`/`<ExtDimensionTypes/>` самозакрыты при
/// пустоте (witnessed `Международный`).
fn emit_coa_item(value: &PropertyValue, decl: &CoaDecl) -> Result<OutElement, String> {
    let node = as_list(value)?;
    if node.len() != coa_node::N {
        return Err(format!(
            "predefined node must be a List of {} for kind ChartOfAccounts, got {} (§1.0)",
            coa_node::N,
            node.len()
        ));
    }
    let dense_flags = |host_tag: &str, truthy: &PropertyValue, paths: &[String]| -> Result<OutElement, String> {
        let truthy = as_list(truthy)?;
        let mut truthy_paths = Vec::with_capacity(truthy.len());
        for t in truthy {
            truthy_paths.push(as_str(t)?);
        }
        if paths.is_empty() {
            return Ok(OutElement::self_closing("", host_tag));
        }
        let mut host = OutElement::branch("", host_tag);
        for p in paths {
            let is_true = truthy_paths.contains(&p.as_str());
            host.push(
                OutElement::leaf("", "Flag", if is_true { "true" } else { "false" })
                    .attr("ref", p.clone()),
            );
        }
        Ok(host)
    };

    let mut el = OutElement::branch("", "Item").attr("id", as_str(&node[coa_node::ID])?);
    el.push(OutElement::leaf("", "Name", as_str(&node[coa_node::NAME])?));
    el.push(text_leaf("Code", as_str(&node[coa_node::CODE])?));
    el.push(text_leaf("Description", as_str(&node[coa_node::DESC])?));
    let at = match &node[coa_node::ACCOUNT_TYPE] {
        PropertyValue::Enum(t) => t.as_str(),
        other => {
            return Err(format!(
                "predefined CoA node: accountType expects Enum, got {:?} (§1.0)",
                other.kind()
            ))
        }
    };
    el.push(OutElement::leaf("", "AccountType", at));
    let off = as_bool(&node[coa_node::OFF_BALANCE])?;
    el.push(OutElement::leaf(
        "",
        "OffBalance",
        if off { "true" } else { "false" },
    ));
    el.push(text_leaf("Order", as_str(&node[coa_node::ORDER])?));
    el.push(dense_flags(
        "AccountingFlags",
        &node[coa_node::ACCOUNTING_FLAGS],
        &decl.flags,
    )?);
    let eds = as_list(&node[coa_node::EXT_DIMENSIONS])?;
    if eds.is_empty() {
        el.push(OutElement::self_closing("", "ExtDimensionTypes"));
    } else {
        let mut host = OutElement::branch("", "ExtDimensionTypes");
        for ed_v in eds {
            let ed = as_list(ed_v)?;
            if ed.len() != coa_node::ED_N {
                return Err(format!(
                    "predefined CoA ED node must be List of {} (§1.0)",
                    coa_node::ED_N
                ));
            }
            let mut e = OutElement::branch("", "ExtDimensionType")
                .attr("name", as_str(&ed[coa_node::ED_CHAR_TYPE])?);
            let turn = as_bool(&ed[coa_node::ED_TURNOVER])?;
            e.push(OutElement::leaf(
                "",
                "Turnover",
                if turn { "true" } else { "false" },
            ));
            e.push(dense_flags(
                "AccountingFlags",
                &ed[coa_node::ED_FLAGS],
                &decl.ed_flags,
            )?);
            host.push(e);
        }
        el.push(host);
    }
    let children = as_list(&node[coa_node::CHILDREN])?;
    if !children.is_empty() {
        let mut host = OutElement::branch("", "ChildItems");
        for c in children {
            host.push(emit_coa_item(c, decl)?);
        }
        el.push(host);
    }
    Ok(el)
}

// --- WRITE --------------------------------------------------------------------------------

/// Write-side mirror: эмитить `predefined` в Designer-сайдкар. EDT (блок едет в дескрипторе)
/// и cf (тело собирает cf-ассемблер) — no-op; пустой/отсутствующий блок — no-op (файла нет).
pub fn write_predefined(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(());
    }
    let Some((field, xsi_type)) = carrier(kind) else {
        return Ok(());
    };
    let predefined = match obj.get(field) {
        Some(v @ PropertyValue::List(l)) if !l.is_empty() => v,
        _ => return Ok(()),
    };
    let path = sidecar_path(descriptor_out).ok_or_else(|| ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: format!("descriptor path has no parent/stem to anchor Ext/{FILE} at"),
    })?;
    // Версия таргета — амбьентный round-trip-таргет (FORMATS.md §1: формат выхода —
    // параметр); реестр всегда несёт witnessed-таргет, страховочный дефолт — SSL.
    let version = formats_designer::common::profile_for(crate::sidecar_version::write_target())
        .map(|p| p.version_value)
        .unwrap_or("2.21");
    let coa_decl = (kind == "ChartOfAccounts").then(|| coa_decl_of(obj));
    let bytes = serialize(kind, xsi_type, predefined, version, coa_decl.as_ref()).map_err(
        |reason| ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason,
        },
    )?;
    crate::form_write::write_file(&path, &bytes)
}

/// `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Predefined.xml`.
fn sidecar_path(descriptor_path: &Path) -> Option<PathBuf> {
    Some(
        descriptor_path
            .parent()?
            .join(descriptor_path.file_stem()?)
            .join("Ext")
            .join(FILE),
    )
}

/// Канон → байты Designer-сайдкара (BOM, CRLF, таб, без хвостового EOL). `version` —
/// значение корневого `version=` («2.20»/«2.21»): на write — таргет
/// ([`crate::sidecar_version::write_target`]), в §1.0-самопроверке read — версия ИСТОЧНИКА.
fn serialize(
    kind: &str,
    xsi_type: &str,
    predefined: &PropertyValue,
    version: &str,
    coa_decl: Option<&CoaDecl>,
) -> Result<Vec<u8>, String> {
    let items = as_list(predefined)?;
    let mut root = OutElement::branch("", "PredefinedData")
        .attr("xmlns", "http://v8.1c.ru/8.3/xcf/predef")
        .attr("xmlns:v8", "http://v8.1c.ru/8.1/data/core")
        .attr("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable")
        .attr("xmlns:xs", "http://www.w3.org/2001/XMLSchema")
        .attr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
        .attr("xsi:type", xsi_type)
        .attr("version", version);
    for item in items {
        if kind == "ChartOfAccounts" {
            let decl = coa_decl
                .ok_or("predefined CoA serialize requires the flag declarations (bug)")?;
            root.push(emit_coa_item(item, decl)?);
        } else {
            root.push(emit_item(kind, item, 0)?);
        }
    }
    Ok(render(&ENVELOPE, &root))
}

/// Текстовый лист: пустой текст → самозакрывающийся (`<Code/>` — конвенция Designer).
fn text_leaf(tag: &str, text: &str) -> OutElement {
    if text.is_empty() {
        OutElement::self_closing("", tag)
    } else {
        OutElement::leaf("", tag, text)
    }
}

/// Эмитировать `<Item>` (DENSE: Name/Code/Description/[Type]/IsFolder/[ChildItems]).
/// `level` — вложенность (0 = корневой `<Item>`): задаёт авто-префикс `d{N}p1` вложенных
/// `<v8:Type>` ([`type_host_depth`]; witnessed d4p1/d6p1/d8p1/d10p1 в ERP CCT-сайдкарах).
fn emit_item(kind: &str, value: &PropertyValue, level: usize) -> Result<OutElement, String> {
    let node = as_list(value)?;
    let n = if is_cct(kind) {
        cct_node::N
    } else {
        cat_node::N
    };
    if node.len() != n {
        return Err(format!(
            "predefined node must be a List of {n} for kind {kind}, got {} (§1.0)",
            node.len()
        ));
    }
    let mut el = OutElement::branch("", "Item").attr("id", as_str(&node[cat_node::ID])?);

    if is_cct(kind) {
        el.push(OutElement::leaf("", "Name", as_str(&node[cct_node::NAME])?));
        // DENSE: пустой код → `<Code/>` (has_code — EDT-разрежённость, Designer её не пишет).
        el.push(text_leaf("Code", as_str(&node[cct_node::CODE])?));
        el.push(text_leaf("Description", as_str(&node[cct_node::DESC])?));
        // Пустой TypeSpec (папка) → self-closing `<Type/>` (witnessed).
        el.push(type_codec::encode_scoped_auto_cfg(
            TypeDialect::Designer,
            "",
            "Type",
            as_type(&node[cct_node::TYPE])?,
            &[],
            Some(type_host_depth(level)),
        )?);
        let is_folder = as_bool(&node[cct_node::IS_FOLDER])?;
        el.push(OutElement::leaf(
            "",
            "IsFolder",
            if is_folder { "true" } else { "false" },
        ));
        let children = as_list(&node[cct_node::CHILDREN])?;
        if !children.is_empty() {
            let mut host = OutElement::branch("", "ChildItems");
            for c in children {
                host.push(emit_item(kind, c, level + 1)?);
            }
            el.push(host);
        }
        return Ok(el);
    }

    el.push(OutElement::leaf("", "Name", as_str(&node[cat_node::NAME])?));
    // Числовой код (code_is_number) — `<Code xsi:type="xs:decimal">цифры</Code>`
    // (ERP-witnessed; текст непустой по построению read-сторон обоих диалектов).
    if as_bool(&node[cat_node::CODE_IS_NUMBER])? {
        el.push(
            OutElement::leaf("", "Code", as_str(&node[cat_node::CODE])?)
                .attr("xsi:type", "xs:decimal"),
        );
    } else {
        el.push(text_leaf("Code", as_str(&node[cat_node::CODE])?));
    }
    el.push(text_leaf("Description", as_str(&node[cat_node::DESC])?));
    let is_folder = as_bool(&node[cat_node::IS_FOLDER])?;
    el.push(OutElement::leaf(
        "",
        "IsFolder",
        if is_folder { "true" } else { "false" },
    ));
    let children = as_list(&node[cat_node::CHILDREN])?;
    if !children.is_empty() {
        let mut host = OutElement::branch("", "ChildItems");
        for c in children {
            host.push(emit_item(kind, c, level + 1)?);
        }
        el.push(host);
    }
    Ok(el)
}

// --- helpers ------------------------------------------------------------------------------

fn as_list(v: &PropertyValue) -> Result<&Vec<PropertyValue>, String> {
    match v {
        PropertyValue::List(l) => Ok(l),
        other => Err(format!("predefined: expected List, got {:?}", other.kind())),
    }
}
fn as_str(v: &PropertyValue) -> Result<&str, String> {
    match v {
        PropertyValue::Str(s) => Ok(s),
        other => Err(format!("predefined: expected Str, got {:?}", other.kind())),
    }
}
fn as_bool(v: &PropertyValue) -> Result<bool, String> {
    match v {
        PropertyValue::Bool(b) => Ok(*b),
        other => Err(format!("predefined: expected Bool, got {:?}", other.kind())),
    }
}
fn as_type(v: &PropertyValue) -> Result<&TypeSpec, String> {
    match v {
        PropertyValue::Type(t) => Ok(t),
        other => Err(format!("predefined: expected Type, got {:?}", other.kind())),
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Designer-сайдкар Catalog с рекурсией (`ChildItems`), пустым `<Code/>` и `<IsFolder>`.
    const CAT_SRC: &str = concat!(
        "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" ",
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ",
        "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
        "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
        "xsi:type=\"CatalogPredefinedItems\" version=\"2.21\">\r\n",
        "\t<Item id=\"8cbaa30d-faab-45ad-880e-84f8b421f448\">\r\n",
        "\t\t<Name>Группа</Name>\r\n",
        "\t\t<Code/>\r\n",
        "\t\t<Description>признак \"Рассмотрено\"</Description>\r\n",
        "\t\t<IsFolder>true</IsFolder>\r\n",
        "\t\t<ChildItems>\r\n",
        "\t\t\t<Item id=\"35dccbf2-dcce-4160-91a0-9697dd21aa79\">\r\n",
        "\t\t\t\t<Name>Телефон</Name>\r\n",
        "\t\t\t\t<Code>000000001</Code>\r\n",
        "\t\t\t\t<Description/>\r\n",
        "\t\t\t\t<IsFolder>false</IsFolder>\r\n",
        "\t\t\t</Item>\r\n",
        "\t\t</ChildItems>\r\n",
        "\t</Item>\r\n",
        "</PredefinedData>"
    );

    /// Byte-exact round-trip Designer-сайдкара (это и есть §1.0-самопроверка на read).
    #[test]
    fn designer_catalog_sidecar_roundtrips_byte_exact() {
        let doc = formats_xml::parse(CAT_SRC.as_bytes()).expect("xml");
        let items: Vec<_> = doc
            .root
            .children
            .iter()
            .map(|c| decode_item("Catalog", c).expect("decode"))
            .collect();
        let pd = PropertyValue::List(items);
        let out = serialize("Catalog", "CatalogPredefinedItems", &pd, "2.21", None).expect("serialize");
        assert_eq!(String::from_utf8(out).unwrap(), CAT_SRC);
    }

    /// Presence-флаги канона = EDT-РАЗРЕЖЕННОСТЬ: пустой `<Description/>` → `has_desc=false`;
    /// `<IsFolder>false</IsFolder>` → `has_folder=false` (EDT оба ОПУСКАЕТ).
    #[test]
    fn presence_flags_mirror_edt_sparsity() {
        let doc = formats_xml::parse(CAT_SRC.as_bytes()).expect("xml");
        let root = decode_item("Catalog", &doc.root.children[0]).expect("decode");
        let PropertyValue::List(node) = &root else {
            panic!("node")
        };
        assert_eq!(node[cat_node::HAS_DESC], PropertyValue::Bool(true));
        assert_eq!(node[cat_node::HAS_FOLDER], PropertyValue::Bool(true));
        let PropertyValue::List(kids) = &node[cat_node::CHILDREN] else {
            panic!("children")
        };
        let PropertyValue::List(kid) = &kids[0] else {
            panic!("kid")
        };
        assert_eq!(kid[cat_node::HAS_DESC], PropertyValue::Bool(false));
        assert_eq!(kid[cat_node::HAS_FOLDER], PropertyValue::Bool(false));
        assert_eq!(kid[cat_node::CODE], PropertyValue::Str("000000001".into()));
    }

    /// ERP-witnessed числовой код (`ВидыИспользованияРабочегоВремени`, codeType=Number,
    /// version 2.20): `<Code xsi:type="xs:decimal">0</Code>` — byte-exact round-trip,
    /// `code_is_number=true` в каноне (зеркалит EDT `core:NumberValue`).
    #[test]
    fn designer_catalog_numeric_code_roundtrips_byte_exact() {
        const NUM_SRC: &str = concat!(
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" ",
            "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ",
            "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
            "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
            "xsi:type=\"CatalogPredefinedItems\" version=\"2.20\">\r\n",
            "\t<Item id=\"dff9951a-9539-4139-889b-a7ba7a9759f6\">\r\n",
            "\t\t<Name>Болезнь</Name>\r\n",
            "\t\t<Code xsi:type=\"xs:decimal\">0</Code>\r\n",
            "\t\t<Description>Болезнь</Description>\r\n",
            "\t\t<IsFolder>false</IsFolder>\r\n",
            "\t</Item>\r\n",
            "</PredefinedData>"
        );
        let doc = formats_xml::parse(NUM_SRC.as_bytes()).expect("xml");
        let items: Vec<_> = doc
            .root
            .children
            .iter()
            .map(|c| decode_item("Catalog", c).expect("decode"))
            .collect();
        let PropertyValue::List(node) = &items[0] else { panic!("node") };
        assert_eq!(node[cat_node::CODE], PropertyValue::Str("0".into()));
        assert_eq!(node[cat_node::CODE_IS_NUMBER], PropertyValue::Bool(true));
        let pd = PropertyValue::List(items);
        let out = serialize("Catalog", "CatalogPredefinedItems", &pd, "2.20", None).expect("serialize");
        assert_eq!(String::from_utf8(out).unwrap(), NUM_SRC);
    }

    /// §1.0: посторонний xsi:type у `<Code>` (или xs:decimal у CCT) — отказ, не догадка.
    #[test]
    fn foreign_code_xsi_type_is_refused() {
        let xml = "<PredefinedData><Item id=\"a\"><Name>Х</Name>\
                   <Code xsi:type=\"xs:int\">1</Code><Description/>\
                   <IsFolder>false</IsFolder></Item></PredefinedData>";
        let doc = formats_xml::parse(xml.as_bytes()).expect("xml");
        let err = decode_item("Catalog", &doc.root.children[0]).unwrap_err();
        assert!(err.contains("unwitnessed xsi:type"), "{err}");
        let err = decode_item("ChartOfCharacteristicTypes", &doc.root.children[0]).unwrap_err();
        assert!(err.contains("unwitnessed xsi:type"), "{err}");
    }

    /// CCT-узел: `<Type>` со ССЫЛОЧНЫМ типом несёт АВТО-префикс `d4p1:` + инлайн-ns (корень
    /// сайдкара `cfg` НЕ объявляет) — byte-exact round-trip.
    #[test]
    fn designer_cct_sidecar_roundtrips_byte_exact() {
        const CCT_SRC: &str = concat!(
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" ",
            "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ",
            "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
            "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
            "xsi:type=\"PlanOfCharacteristicKindPredefinedItems\" version=\"2.21\">\r\n",
            "\t<Item id=\"527c98a3-3bfc-4bf9-8ec7-0247fdfa7a29\">\r\n",
            "\t\t<Name>Раздел</Name>\r\n",
            "\t\t<Code/>\r\n",
            "\t\t<Description>Раздел дат</Description>\r\n",
            "\t\t<Type>\r\n",
            "\t\t\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">",
            "d4p1:ChartOfCharacteristicTypesRef.РазделыДатЗапретаИзменения</v8:Type>\r\n",
            "\t\t</Type>\r\n",
            "\t\t<IsFolder>false</IsFolder>\r\n",
            "\t</Item>\r\n",
            "</PredefinedData>"
        );
        let doc = formats_xml::parse(CCT_SRC.as_bytes()).expect("xml");
        let items: Vec<_> = doc
            .root
            .children
            .iter()
            .map(|c| decode_item("ChartOfCharacteristicTypes", c).expect("decode"))
            .collect();
        let pd = PropertyValue::List(items);
        let out = serialize(
            "ChartOfCharacteristicTypes",
            "PlanOfCharacteristicKindPredefinedItems",
            &pd,
            "2.21",
            None,
        )
        .expect("serialize");
        assert_eq!(String::from_utf8(out).unwrap(), CCT_SRC);
    }

    /// ERP-witnessed CCT-шейп (`СтатьиАктивовПассивов`, version 2.20): непустой `<Code>`,
    /// ПАПКА (`IsFolder=true`, пустой `<Type/>`) с `<ChildItems>`, вложенный ref-тип с
    /// авто-префиксом `d6p1` (глубина 6 на уровне 1) — byte-exact round-trip.
    #[test]
    fn designer_cct_sidecar_with_codes_and_folders_roundtrips_byte_exact() {
        const CCT_ERP_SRC: &str = concat!(
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" ",
            "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ",
            "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
            "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
            "xsi:type=\"PlanOfCharacteristicKindPredefinedItems\" version=\"2.20\">\r\n",
            "\t<Item id=\"99bf279f-8b30-43e2-bd28-73db4e5d0176\">\r\n",
            "\t\t<Name>СтатьиУправленческогоБаланса</Name>\r\n",
            "\t\t<Code>00000000067</Code>\r\n",
            "\t\t<Description>Статьи управленческого баланса</Description>\r\n",
            "\t\t<Type/>\r\n",
            "\t\t<IsFolder>true</IsFolder>\r\n",
            "\t\t<ChildItems>\r\n",
            "\t\t\t<Item id=\"db0609a2-f673-4191-9d5d-3344d0631c27\">\r\n",
            "\t\t\t\t<Name>УпрБалансКапитализацияНМАиНИОКР</Name>\r\n",
            "\t\t\t\t<Code>00000000051</Code>\r\n",
            "\t\t\t\t<Description>Капитализация НМА и НИОКР</Description>\r\n",
            "\t\t\t\t<Type>\r\n",
            "\t\t\t\t\t<v8:Type xmlns:d6p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">",
            "d6p1:CatalogRef.НематериальныеАктивы</v8:Type>\r\n",
            "\t\t\t\t</Type>\r\n",
            "\t\t\t\t<IsFolder>false</IsFolder>\r\n",
            "\t\t\t</Item>\r\n",
            "\t\t</ChildItems>\r\n",
            "\t</Item>\r\n",
            "</PredefinedData>"
        );
        let doc = formats_xml::parse(CCT_ERP_SRC.as_bytes()).expect("xml");
        let items: Vec<_> = doc
            .root
            .children
            .iter()
            .map(|c| decode_item("ChartOfCharacteristicTypes", c).expect("decode"))
            .collect();
        // Канонический узел: код/папка/дети смоделированы (has_code=EDT-разрежённость).
        let PropertyValue::List(root_node) = &items[0] else {
            panic!("node")
        };
        assert_eq!(root_node[cct_node::HAS_CODE], PropertyValue::Bool(true));
        assert_eq!(
            root_node[cct_node::CODE],
            PropertyValue::Str("00000000067".into())
        );
        assert_eq!(root_node[cct_node::HAS_FOLDER], PropertyValue::Bool(true));
        assert_eq!(root_node[cct_node::IS_FOLDER], PropertyValue::Bool(true));
        let pd = PropertyValue::List(items);
        let out = serialize(
            "ChartOfCharacteristicTypes",
            "PlanOfCharacteristicKindPredefinedItems",
            &pd,
            "2.20",
            None,
        )
        .expect("serialize");
        assert_eq!(String::from_utf8(out).unwrap(), CCT_ERP_SRC);
    }

    /// ERP-witnessed CoA-сайдкар (`Международный`, version 2.20, byte-form реального
    /// файла): DENSE AccountType/OffBalance/Order, `<Flag ref>false`, самозакрытый
    /// `<ExtDimensionTypes/>` — byte-exact round-trip; канон несёт лишь ИСТИННЫЕ флаги
    /// (здесь — ни одного).
    #[test]
    fn designer_coa_sidecar_roundtrips_byte_exact() {
        const COA_SRC: &str = concat!(
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" ",
            "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ",
            "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
            "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
            "xsi:type=\"ChartOfAccountsPredefinedItems\" version=\"2.20\">\r\n",
            "\t<Item id=\"30909122-fcb9-4ab6-a199-f4a7b059e177\">\r\n",
            "\t\t<Name>Служебный</Name>\r\n",
            "\t\t<Code>00000</Code>\r\n",
            "\t\t<Description>Служебный</Description>\r\n",
            "\t\t<AccountType>ActivePassive</AccountType>\r\n",
            "\t\t<OffBalance>false</OffBalance>\r\n",
            "\t\t<Order>00000</Order>\r\n",
            "\t\t<AccountingFlags>\r\n",
            "\t\t\t<Flag ref=\"ChartOfAccounts.Международный.AccountingFlag.Валютный\">false</Flag>\r\n",
            "\t\t\t<Flag ref=\"ChartOfAccounts.Международный.AccountingFlag.УчетПоПодразделениям\">false</Flag>\r\n",
            "\t\t\t<Flag ref=\"ChartOfAccounts.Международный.AccountingFlag.УчетПоНаправлениямДеятельности\">false</Flag>\r\n",
            "\t\t\t<Flag ref=\"ChartOfAccounts.Международный.AccountingFlag.Количественный\">false</Flag>\r\n",
            "\t\t</AccountingFlags>\r\n",
            "\t\t<ExtDimensionTypes/>\r\n",
            "\t</Item>\r\n",
            "</PredefinedData>"
        );
        let doc = formats_xml::parse(COA_SRC.as_bytes()).expect("xml");
        let items: Vec<_> = doc
            .root
            .children
            .iter()
            .map(|c| decode_coa_item(c).expect("decode"))
            .collect();
        let PropertyValue::List(node) = &items[0] else {
            panic!("node")
        };
        assert_eq!(
            node[coa_node::ACCOUNT_TYPE],
            PropertyValue::Enum(Token::new("ActivePassive"))
        );
        assert_eq!(node[coa_node::OFF_BALANCE], PropertyValue::Bool(false));
        assert_eq!(node[coa_node::ORDER], PropertyValue::Str("00000".into()));
        assert_eq!(
            node[coa_node::ACCOUNTING_FLAGS],
            PropertyValue::List(Vec::new()),
            "канон несёт лишь истинные флаги"
        );
        let decl = CoaDecl {
            flags: [
                "Валютный",
                "УчетПоПодразделениям",
                "УчетПоНаправлениямДеятельности",
                "Количественный",
            ]
            .iter()
            .map(|f| format!("ChartOfAccounts.Международный.AccountingFlag.{f}"))
            .collect(),
            ed_flags: Vec::new(),
        };
        let pd = PropertyValue::List(items);
        let out = serialize(
            "ChartOfAccounts",
            "ChartOfAccountsPredefinedItems",
            &pd,
            "2.20",
            Some(&decl),
        )
        .expect("serialize");
        assert_eq!(String::from_utf8(out).unwrap(), COA_SRC);
    }
}
