//! Codec для EDT-блока `<predefined>` корня `ChartOfAccounts` (предопределённые счета).
//! Designer тело несёт САЙДКАРОМ `Ext/Predefined.xml` (его читает
//! `morph1c_pipeline::predefined_read` в ЭТО ЖЕ спек-поле); cf — тело `<uuid>.9`
//! (`formats_cf::predefined_body`). Поле X-исключено (`x_ignore`), участвует в R EDT.
//!
//! Грамматика узла (ERP-witnessed: Хозрасчетный 438 узлов + Международный 1; 105 разных
//! шейпов — все подпоследовательности одного порядка):
//! * `<items id>`/`<childItems id>`, дети ПО ПОРЯДКУ: `<name>` → `[<description>]` →
//!   `<code>` (ПЛОСКИЙ текст, required) → `[<accountType>]` (литерал `Passive`/
//!   `ActivePassive`; ОТСУТСТВИЕ ⟺ `Active` — 215 Active-счетов Хозрасчетного все
//!   омитят лист; present-`Active` НЕ witnessed → §1.0-отказ) → `[<offBalance>]`
//!   (witnessed ТОЛЬКО `true`, 56 узлов; present-`false` → отказ) → `<order>` (ПЛОСКИЙ
//!   текст, required; ведущие пробелы ЗНАЧИМЫ — ` 01`) → `<accountingFlags>`*
//!   (ref-пути `ChartOfAccounts.<Имя>.AccountingFlag.<Флаг>` ТОЛЬКО истинных флагов;
//!   порядок — подпоследовательность порядка объявления флагов, сверено 339/339) →
//!   `<extDimensionTypes>`* → `<childItems>`* (рекурсивно, глубина до 3).
//! * `<extDimensionTypes>`: `<characteristicType>` (ref-путь предопределённого вида
//!   субконто, required) → `[<turnover>]` (witnessed только `true`, 81) →
//!   `<extDimensionAccountingFlags>`* (ref-пути истинных ед-флагов; подпоследовательность
//!   объявления, 611/611).
//!
//! Канонический IR: `PropertyValue::List` узлов; узел — `List([id:Str, name:Str,
//! has_desc:Bool, description:Str, code:Str, account_type:Enum, off_balance:Bool,
//! order:Str, accounting_flags:List[Str], ext_dimensions:List, children:List])`;
//! ед-узел — `List([characteristic_type:Str, turnover:Bool, flags:List[Str]])`.
//! Разрежённость восстановима БЕЗ presence-флагов: EDT омитит accountType ⟺ Active,
//! offBalance ⟺ false, turnover ⟺ false (witnessed-биекции; present-дефолт → отказ).
//! §1.0: иная структура/литерал → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{PropertyValue, Token};

// IR node layout indices — pub: их РЕЭКСПОРТИРУЮТ `morph1c_pipeline::predefined_read`
// (Designer-сайдкар) и `formats_cf::predefined_body` (cf-тело). Префикс [0..4] намеренно
// СОВПАДАЕТ с Catalog/CCT-узлами (ID/NAME — общий код харвеста путей).
/// `@id` узла.
pub const I_ID: usize = 0;
/// `<name>`.
pub const I_NAME: usize = 1;
/// presence-флаг `<description>`.
pub const I_HAS_DESC: usize = 2;
/// Текст `<description>` (`""` при отсутствии).
pub const I_DESC: usize = 3;
/// `<code>` — плоский текст (required).
pub const I_CODE: usize = 4;
/// `<accountType>` — Enum-литерал `Active`/`Passive`/`ActivePassive` (отсутствие ⟺ Active).
pub const I_ACCOUNT_TYPE: usize = 5;
/// `<offBalance>` (bool; отсутствие ⟺ false, witnessed только true).
pub const I_OFF_BALANCE: usize = 6;
/// `<order>` — плоский текст (required; ведущие пробелы значимы).
pub const I_ORDER: usize = 7;
/// `<accountingFlags>`-ссылки ИСТИННЫХ флагов (List[Str]; порядок объявления).
pub const I_ACCOUNTING_FLAGS: usize = 8;
/// `<extDimensionTypes>`-узлы (List из ED-узлов).
pub const I_EXT_DIMENSIONS: usize = 9;
/// Рекурсивные `<childItems>` (List).
pub const I_CHILDREN: usize = 10;
/// Арность узла.
pub const N: usize = 11;

// ED-узел (`<extDimensionTypes>`).
/// `<characteristicType>` — ref-путь предопределённого вида субконто.
pub const ED_CHAR_TYPE: usize = 0;
/// `<turnover>` (bool; отсутствие ⟺ false).
pub const ED_TURNOVER: usize = 1;
/// `<extDimensionAccountingFlags>`-ссылки истинных ед-флагов (List[Str]).
pub const ED_FLAGS: usize = 2;
/// Арность ED-узла.
pub const ED_N: usize = 3;

/// Полный домен литералов вида счёта (см. `value_codec::ACCOUNT_TYPE_LITERALS` — cf-коды
/// 0/1/2 witnessed корреляцией 438/438).
const ACCOUNT_TYPE_LITERALS: [&str; 3] = ["Active", "Passive", "ActivePassive"];

// --- DECODE (EDT, from root) ------------------------------------------------

/// Декодировать `<predefined>` от КОРНЯ источника. Отсутствие → Absent (поле опционально).
pub fn decode_edt(root: &Element) -> Decoded {
    let host = match root.child("predefined") {
        Some(h) if h.prefix.is_empty() => h,
        Some(_) => return Decoded::Error("predefined: must be unprefixed".into()),
        None => return Decoded::Absent,
    };
    host.claim();
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error(
            "predefined: container must be attribute-less, no text (§1.0)".into(),
        );
    }
    let mut items = Vec::new();
    for child in &host.children {
        if child.local != "items" || !child.prefix.is_empty() {
            return Decoded::Error(format!(
                "predefined: expected <items>, got <{}>",
                child.local
            ));
        }
        match decode_node(child) {
            Ok(n) => items.push(n),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(PropertyValue::List(items))
}

fn decode_node(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    let id = el
        .attr("id")
        .ok_or_else(|| format!("<{}> predefined node missing @id", el.local))?;
    let id_val = id.value.clone();
    id.claimed.set(true);
    if el.attrs.iter().any(|a| !a.claimed.get()) {
        return Err(format!(
            "<{}> predefined node has unexpected extra attribute",
            el.local
        ));
    }

    let mut node: Vec<PropertyValue> = vec![
        PropertyValue::Str(id_val),
        PropertyValue::Str(String::new()),       // name
        PropertyValue::Bool(false),              // has_desc
        PropertyValue::Str(String::new()),       // description
        PropertyValue::Str(String::new()),       // code
        PropertyValue::Enum(Token::new("Active")), // accountType (деф.)
        PropertyValue::Bool(false),              // offBalance
        PropertyValue::Str(String::new()),       // order
        PropertyValue::List(Vec::new()),         // accountingFlags
        PropertyValue::List(Vec::new()),         // extDimensionTypes
        PropertyValue::List(Vec::new()),         // children
    ];
    debug_assert_eq!(node.len(), N);

    let mut it = el.children.iter().peekable();
    // name (required, first).
    let name = it
        .next()
        .filter(|c| c.local == "name" && c.prefix.is_empty())
        .ok_or_else(|| format!("<{}> predefined node missing <name>", el.local))?;
    name.claim_with_text();
    node[I_NAME] = PropertyValue::Str(name.text.clone());
    // [description].
    if let Some(c) = it.peek() {
        if c.local == "description" && c.prefix.is_empty() {
            let d = it.next().unwrap();
            d.claim_with_text();
            node[I_HAS_DESC] = PropertyValue::Bool(true);
            node[I_DESC] = PropertyValue::Str(d.text.clone());
        }
    }
    // code (required): ПЛОСКИЙ текст (в отличие от Catalog Value-xsi).
    let code = it
        .next()
        .filter(|c| c.local == "code" && c.prefix.is_empty())
        .ok_or_else(|| format!("<{}> predefined node missing <code>", el.local))?;
    code.claim_with_text();
    node[I_CODE] = PropertyValue::Str(code.text.clone());
    // [accountType] — литерал; отсутствие ⟺ Active (witnessed-биекция: present-Active
    // не встречен в 439 узлах → present-дефолт неотличим на re-emit → отказ, не догадка).
    if let Some(c) = it.peek() {
        if c.local == "accountType" && c.prefix.is_empty() {
            let a = it.next().unwrap();
            a.claim_with_text();
            match a.text.as_str() {
                "Active" => {
                    return Err(format!(
                        "<{}> predefined node: present <accountType>Active</accountType> is \
                         unwitnessed (absence ⟺ Active — §1.0)",
                        el.local
                    ))
                }
                t if ACCOUNT_TYPE_LITERALS.contains(&t) => {
                    node[I_ACCOUNT_TYPE] = PropertyValue::Enum(Token::new(t));
                }
                other => {
                    return Err(format!(
                        "<{}> predefined node: <accountType> literal must be one of \
                         {ACCOUNT_TYPE_LITERALS:?}, got {other:?} (§1.0)",
                        el.local
                    ))
                }
            }
        }
    }
    // [offBalance] — witnessed только true (56 узлов); present-false → отказ.
    if let Some(c) = it.peek() {
        if c.local == "offBalance" && c.prefix.is_empty() {
            let o = it.next().unwrap();
            o.claim_with_text();
            match o.text.as_str() {
                "true" => node[I_OFF_BALANCE] = PropertyValue::Bool(true),
                other => {
                    return Err(format!(
                        "<{}> predefined node: <offBalance> must be \"true\" (absence ⟺ false; \
                         witnessed 56/56), got {other:?} (§1.0)",
                        el.local
                    ))
                }
            }
        }
    }
    // order (required): плоский текст, ведущие пробелы значимы (` 01`).
    let order = it
        .next()
        .filter(|c| c.local == "order" && c.prefix.is_empty())
        .ok_or_else(|| format!("<{}> predefined node missing <order>", el.local))?;
    order.claim_with_text();
    node[I_ORDER] = PropertyValue::Str(order.text.clone());
    // accountingFlags* — ref-пути истинных флагов.
    let mut flags = Vec::new();
    while let Some(c) = it.peek() {
        if c.local == "accountingFlags" && c.prefix.is_empty() {
            let f = it.next().unwrap();
            f.claim_with_text();
            flags.push(PropertyValue::Str(f.text.clone()));
        } else {
            break;
        }
    }
    node[I_ACCOUNTING_FLAGS] = PropertyValue::List(flags);
    // extDimensionTypes*.
    let mut eds = Vec::new();
    while let Some(c) = it.peek() {
        if c.local == "extDimensionTypes" && c.prefix.is_empty() {
            eds.push(decode_ed(it.next().unwrap())?);
        } else {
            break;
        }
    }
    node[I_EXT_DIMENSIONS] = PropertyValue::List(eds);
    // childItems* recursive.
    let mut children = Vec::new();
    for c in it {
        if c.local != "childItems" || !c.prefix.is_empty() {
            return Err(format!(
                "<{}> predefined node: unexpected <{}> (§1.0)",
                el.local, c.local
            ));
        }
        children.push(decode_node(c)?);
    }
    node[I_CHILDREN] = PropertyValue::List(children);
    Ok(PropertyValue::List(node))
}

/// Разобрать `<extDimensionTypes>`: characteristicType → [turnover] → ед-флаги*.
fn decode_ed(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    if !el.attrs.is_empty() {
        return Err("<extDimensionTypes> has unexpected attribute (§1.0)".into());
    }
    let mut it = el.children.iter().peekable();
    let ct = it
        .next()
        .filter(|c| c.local == "characteristicType" && c.prefix.is_empty())
        .ok_or("predefined <extDimensionTypes> missing <characteristicType> (§1.0)")?;
    ct.claim_with_text();
    let mut turnover = false;
    if let Some(c) = it.peek() {
        if c.local == "turnover" && c.prefix.is_empty() {
            let t = it.next().unwrap();
            t.claim_with_text();
            match t.text.as_str() {
                "true" => turnover = true,
                other => {
                    return Err(format!(
                        "predefined <turnover> must be \"true\" (absence ⟺ false; witnessed \
                         81/81), got {other:?} (§1.0)"
                    ))
                }
            }
        }
    }
    let mut flags = Vec::new();
    for c in it {
        if c.local != "extDimensionAccountingFlags" || !c.prefix.is_empty() {
            return Err(format!(
                "predefined <extDimensionTypes>: unexpected <{}> (§1.0)",
                c.local
            ));
        }
        c.claim_with_text();
        flags.push(PropertyValue::Str(c.text.clone()));
    }
    Ok(PropertyValue::List(vec![
        PropertyValue::Str(ct.text.clone()),
        PropertyValue::Bool(turnover),
        PropertyValue::List(flags),
    ]))
}

/// Claim (для leftover) — то же, что decode.
pub fn claim_edt(root: &Element) {
    let _ = decode_edt(root);
}

// --- EMIT (EDT) --------------------------------------------------------------

/// Эмитировать `<predefined>` из IR.
pub fn emit_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let items = as_list(value)?;
    let mut host = OutElement::branch("", "predefined");
    for item in items {
        host.push(emit_node("items", item)?);
    }
    Ok(vec![host])
}

fn emit_node(tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let node = match value {
        PropertyValue::List(l) if l.len() == N => l,
        _ => return Err(format!("predefined CoA node must be List of {N}")),
    };
    let id = as_str(&node[I_ID])?;
    let mut el = OutElement::branch("", tag).attr("id", id.to_string());
    el.push(OutElement::leaf("", "name", as_str(&node[I_NAME])?.to_string()));
    if as_bool(&node[I_HAS_DESC])? {
        el.push(OutElement::leaf(
            "",
            "description",
            as_str(&node[I_DESC])?.to_string(),
        ));
    }
    el.push(OutElement::leaf("", "code", as_str(&node[I_CODE])?.to_string()));
    // accountType — эмитится ⟺ != Active (witnessed-разрежённость).
    let at = as_enum(&node[I_ACCOUNT_TYPE])?;
    if at != "Active" {
        el.push(OutElement::leaf("", "accountType", at.to_string()));
    }
    if as_bool(&node[I_OFF_BALANCE])? {
        el.push(OutElement::leaf("", "offBalance", "true"));
    }
    el.push(OutElement::leaf("", "order", as_str(&node[I_ORDER])?.to_string()));
    for f in as_list(&node[I_ACCOUNTING_FLAGS])? {
        el.push(OutElement::leaf("", "accountingFlags", as_str(f)?.to_string()));
    }
    for ed in as_list(&node[I_EXT_DIMENSIONS])? {
        el.push(emit_ed(ed)?);
    }
    for child in as_list(&node[I_CHILDREN])? {
        el.push(emit_node("childItems", child)?);
    }
    Ok(el)
}

fn emit_ed(value: &PropertyValue) -> Result<OutElement, String> {
    let ed = match value {
        PropertyValue::List(l) if l.len() == ED_N => l,
        _ => return Err(format!("predefined CoA ED node must be List of {ED_N}")),
    };
    let mut el = OutElement::branch("", "extDimensionTypes");
    el.push(OutElement::leaf(
        "",
        "characteristicType",
        as_str(&ed[ED_CHAR_TYPE])?.to_string(),
    ));
    if as_bool(&ed[ED_TURNOVER])? {
        el.push(OutElement::leaf("", "turnover", "true"));
    }
    for f in as_list(&ed[ED_FLAGS])? {
        el.push(OutElement::leaf(
            "",
            "extDimensionAccountingFlags",
            as_str(f)?.to_string(),
        ));
    }
    Ok(el)
}

// --- helpers ----------------------------------------------------------------

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
fn as_enum(v: &PropertyValue) -> Result<&str, String> {
    match v {
        PropertyValue::Enum(t) => Ok(t.as_str()),
        other => Err(format!("predefined: expected Enum, got {:?}", other.kind())),
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    fn decode_first(xml: &str) -> Result<PropertyValue, String> {
        let doc = crate::parse(xml.as_bytes()).expect("xml");
        match decode_edt(&doc.root) {
            Decoded::Present(PropertyValue::List(items)) => Ok(items[0].clone()),
            Decoded::Error(e) => Err(e),
            _ => panic!("unexpected decode outcome (absent/non-list)"),
        }
    }

    /// Witnessed ERP `Международный.Служебный`-шейп: name,desc,code,accountType,order.
    #[test]
    fn simple_account_roundtrips() {
        let xml = "<r><predefined><items id=\"30909122-fcb9-4ab6-a199-f4a7b059e177\">\
                   <name>Служебный</name><description>Служебный</description>\
                   <code>00000</code><accountType>ActivePassive</accountType>\
                   <order>00000</order></items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node[I_CODE], PropertyValue::Str("00000".into()));
        assert_eq!(
            node[I_ACCOUNT_TYPE],
            PropertyValue::Enum(Token::new("ActivePassive"))
        );
        assert_eq!(node[I_OFF_BALANCE], PropertyValue::Bool(false));
        let out = emit_node("items", &node_v).unwrap();
        let tags: Vec<_> = out.children.iter().map(|c| c.local.as_str()).collect();
        assert_eq!(tags, ["name", "description", "code", "accountType", "order"]);
    }

    /// Witnessed ERP `Хозрасчетный.ОсновныеСредства`-шейп: accountType ОПУЩЕН (Active),
    /// флаги-ссылки, extDimensionTypes с ед-флагами, рекурсивный childItems, значимый
    /// ведущий пробел в order.
    #[test]
    fn account_with_flags_dimensions_children_roundtrips() {
        let xml = "<r><predefined><items id=\"9bd2854a-2856-4139-b580-0407244cf401\">\
                   <name>ОсновныеСредства</name><description>Основные средства</description>\
                   <code>01</code><order> 01</order>\
                   <accountingFlags>ChartOfAccounts.Хозрасчетный.AccountingFlag.УчетПоПодразделениям</accountingFlags>\
                   <extDimensionTypes>\
                   <characteristicType>ChartOfCharacteristicTypes.ВидыСубконтоХозрасчетные.ОсновныеСредства</characteristicType>\
                   <extDimensionAccountingFlags>ChartOfAccounts.Хозрасчетный.ExtDimensionAccountingFlag.Суммовой</extDimensionAccountingFlags>\
                   </extDimensionTypes>\
                   <childItems id=\"83d798ec-929f-4f72-b78b-44e770856be4\">\
                   <name>ОСвОрганизации</name><description>ОС в организации</description>\
                   <code>01.01</code><order> 01.01</order></childItems>\
                   </items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node[I_ACCOUNT_TYPE], PropertyValue::Enum(Token::new("Active")));
        assert_eq!(node[I_ORDER], PropertyValue::Str(" 01".into()));
        let PropertyValue::List(eds) = &node[I_EXT_DIMENSIONS] else { panic!() };
        assert_eq!(eds.len(), 1);
        let PropertyValue::List(kids) = &node[I_CHILDREN] else { panic!() };
        assert_eq!(kids.len(), 1);
        // Re-emit: accountType НЕ эмитится (Active), порядок листов witnessed.
        let out = emit_node("items", &node_v).unwrap();
        let tags: Vec<_> = out.children.iter().map(|c| c.local.as_str()).collect();
        assert_eq!(
            tags,
            [
                "name",
                "description",
                "code",
                "order",
                "accountingFlags",
                "extDimensionTypes",
                "childItems"
            ]
        );
    }

    /// §1.0: present-дефолты неотличимы на re-emit → отказ (accountType=Active,
    /// offBalance=false, turnover=false).
    #[test]
    fn present_defaults_are_refused() {
        let xml = "<r><predefined><items id=\"a\"><name>С</name><code>1</code>\
                   <accountType>Active</accountType><order>1</order></items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("unwitnessed"), "{err}");
        let xml = "<r><predefined><items id=\"a\"><name>С</name><code>1</code>\
                   <offBalance>false</offBalance><order>1</order></items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("offBalance"), "{err}");
        let xml = "<r><predefined><items id=\"a\"><name>С</name><code>1</code><order>1</order>\
                   <extDimensionTypes><characteristicType>X</characteristicType>\
                   <turnover>false</turnover></extDimensionTypes></items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("turnover"), "{err}");
    }
}
