//! Codec для `Value` ([`PropertyValue::Value`] / [`ValueSpec`]) — nullable-скаляр с
//! xsi-типизацией (`MinValue`/`MaxValue`/`FillValue` и пр.). Контейнерный кодек с
//! зависящей-от-диалекта кодировкой xsi-типа.
//!
//! `Value` 1С — nullable-скаляр известного xsi-вида ([`ValueScalarKind`]). Хост-элемент
//! присутствует ВСЕГДА (это значение, а не дефолт-омиссия). Оба формата кодируют ОДНУ
//! логическую структуру, расходясь структурно. Полный домен (сверено по SSL
//! InformationRegister-корпусу, 6 видов):
//!
//! | вид | EDT xsi:type | EDT нагрузка | Designer xsi | Designer нагрузка |
//! |-----|--------------|--------------|--------------|-------------------|
//! | Undefined | `core:UndefinedValue` | self-close | `xsi:nil="true"` | self-close |
//! | Str       | `core:StringValue`   | self-close(="") / `<value>t</value>` | `xs:string` | self-close(="") / текст |
//! | Bool      | `core:BooleanValue`  | self-close(=false) / `<value>true</value>` | `xs:boolean` | `false`/`true` |
//! | Number    | `core:NumberValue`   | `<value>n</value>` | `xs:decimal` | текст |
//! | Date      | `core:DateValue`     | `<value>iso</value>` | `xs:dateTime` | текст |
//! | Reference | `core:ReferenceValue`| self-close(="") / `<value>r</value>` | `xr:DesignTimeRef` | self-close(="") / текст |
//! | AccountType | `form:AccountTypeValue` | `<value>лит</value>` | `ent:AccountType` | текст |
//!
//! AccountType — ERP-witnessed (`fillValue` std-атрибута `Type` плана счетов Хозрасчетный:
//! EDT `<fillValue xsi:type="form:AccountTypeValue"><value>ActivePassive</value></fillValue>`
//! ⇔ Designer `<xr:FillValue xsi:type="ent:AccountType">ActivePassive</xr:FillValue>`).
//! Литералы — enum вида счёта: `Active`/`Passive`/`ActivePassive` (полный домен witnessed
//! в predefined-телах CoA, 438/438 корреляция с cf-кодами 0/1/2); иной литерал → §1.0-отказ.
//! NB `form:`-ns на КОРНЕ .mdo эмитится declare-iff-used (`root_extra_namespaces` EDT-writer'а
//! фильтрует по `tree_uses_prefix`) — Хозрасчетный несёт `xmlns:form`, Международный нет.
//!
//! READ обоих диалектов даёт ИДЕНТИЧНЫЙ канонический [`ValueSpec`] (X by construction):
//! `kind`+`scalar` одинаковы. WRITE детерминированно восстанавливает байты конвенции
//! формата (R byte-exact). EDT self-close-формы ⇔ «нулевое значение» вида (Str→"",
//! Bool→false, Reference→""); Designer self-close лишь для nil/Str-""/Reference-"".
//!
//! PRESENCE-OF-DEFAULT (субстрат): пустой Str/Reference-скаляр имеет ДВЕ EDT-байт-формы,
//! обе = «пустая строка» — self-close host (разрежённая/дефолтная) И явный пустой
//! `<value></value>`-ребёнок (present-of-default, витнессится в ERP `fillValue`). Биекция
//! «дефолт=омиссия» тут ломается (одно значение в двух дрессах ВНУТРИ одного формата), и
//! byte-exact R требует их различать. Модель: явный `<value></value>` ⇒ `scalar: None`
//! (байт-форм-маркер, валиден лишь при не-`Undefined` `kind`); self-close ⇒ канонический
//! `Some(Str(""))`. Так пер-формат R держится, а §1.6 не страдает: cf/Designer «пустую
//! строку» дают `Some(Str(""))` (никогда `None`), а `{Str/Reference, None}` — EDT-only
//! витнессированная форма, не встречающаяся в X-корпусе (SSL `<value></value>` идёт через
//! `predefined`, не сюда; у ERP нет cf/Designer-пары). Аддитивно: НИ один прочий сайт
//! конструирования `ValueSpec` не меняется. См. [`decode_edt_str_ref`].
//!
//! §1.0: незнакомый xsi-тип/лишний атрибут/лишний ребёнок/чужой ns → типизированная
//! ОШИБКА, никогда silent-drop/guess/passthrough. Домен видов — ровно засвидетельствованный.

use morph1c_core::ir::value::{PropertyValue, ValueScalarKind, ValueSpec};

use crate::descriptor::Element;
use crate::emit::OutElement;

/// Диалект Value-проекции: спеллинг xsi-типа + расположение скаляра.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueDialect {
    /// EDT: `xsi:type="core:…Value"`; непустой скаляр — в `<value>`-ребёнке.
    Edt,
    /// Designer: `xsi:nil="true"` (Undefined) / `xsi:type="xs:…"|xr:DesignTimeRef`;
    /// скаляр — в тексте элемента.
    Designer,
}

const XSI_TYPE: &str = "xsi:type";
const XSI_NIL: &str = "xsi:nil";

// EDT xsi-типы (`core:`-prefixed).
const EDT_UNDEFINED: &str = "core:UndefinedValue";
const EDT_STRING: &str = "core:StringValue";
const EDT_BOOLEAN: &str = "core:BooleanValue";
const EDT_NUMBER: &str = "core:NumberValue";
const EDT_DATE: &str = "core:DateValue";
const EDT_REFERENCE: &str = "core:ReferenceValue";
/// Пустое ОписаниеТипов (EDT: `<value/>`-ребёнок пуст). См. [`ValueScalarKind::TypeDescription`].
const EDT_TYPE_DESCRIPTION: &str = "core:TypeDescriptionValue";
/// Вид счёта (EDT: `<value>ЛИТЕРАЛ</value>`; ns `form` = `http://g5.1c.ru/v8/dt/form`).
const EDT_ACCOUNT_TYPE: &str = "form:AccountTypeValue";
/// Список значений (EDT: пустой self-close `<value xsi:type="core:ValueList"/>`). Непустой
/// layout не витнессирован. См. [`ValueScalarKind::ValueList`].
const EDT_VALUE_LIST: &str = "core:ValueList";
/// Литерал вида счёта, который EDT ОПУСКАЕТ (self-close `form:AccountTypeValue`, cf-код 0):
/// ERP-witness Хозрасчетный.ФормаСчета choiceList — «Активный» несёт пустой `<value/>`, а
/// «Пассивный»/«Активный/Пассивный» — явный `<value>Passive|ActivePassive</value>`. Designer
/// эмитит литерал всегда (текст). Симметрия Boolean (self-close ⟺ дефолт).
const ACCOUNT_TYPE_OMITTED: &str = "Active";

// Designer xsi-типы.
const DES_STRING: &str = "xs:string";
const DES_BOOLEAN: &str = "xs:boolean";
const DES_DECIMAL: &str = "xs:decimal";
const DES_DATETIME: &str = "xs:dateTime";
const DES_REFERENCE: &str = "xr:DesignTimeRef";
/// Пустое ОписаниеТипов (Designer: self-close, без текста). См. [`ValueScalarKind::TypeDescription`].
const DES_TYPE_DESCRIPTION: &str = "v8:TypeDescription";
/// Список значений (Designer: self-close, без текста). См. [`ValueScalarKind::ValueList`].
const DES_VALUE_LIST: &str = "xr:ValueList";
/// Вид счёта (Designer: текст-литерал; ns `ent` объявлен на корне Designer-дескриптора).
const DES_ACCOUNT_TYPE: &str = "ent:AccountType";

/// Полный домен литералов вида счёта (enum платформы; witnessed 438/438 предопределённых
/// счетов Хозрасчетного + fillValue). §1.0: иной литерал → отказ.
const ACCOUNT_TYPE_LITERALS: [&str; 3] = ["Active", "Passive", "ActivePassive"];

fn require_account_type_literal(text: &str) -> Result<(), String> {
    if ACCOUNT_TYPE_LITERALS.contains(&text) {
        return Ok(());
    }
    Err(format!(
        "Value: AccountType literal must be one of {ACCOUNT_TYPE_LITERALS:?}, got {text:?} (§1.0)"
    ))
}

mod decode;
mod encode;

pub(crate) use decode::*;
pub(crate) use encode::*;

// --- DECODE -----------------------------------------------------------------

/// Разобрать host-элемент в канонический [`ValueSpec`] (строго-тотально). Хост уже
/// claimed вызывающим; здесь клеймится xsi-атрибут и (для нагрузки) `<value>`-ребёнок
/// (EDT) либо текст (Designer). Любой неизвестный xsi-тип/лишний атрибут/ребёнок → `Err`.
pub fn decode(dialect: ValueDialect, host: &Element) -> Result<PropertyValue, String> {
    match dialect {
        ValueDialect::Edt => decode_edt(host),
        ValueDialect::Designer => decode_designer(host),
    }
}

// --- CLAIM (согласован с decode, §1.0 B1) -----------------------------------

/// Claim РОВНО те узлы, что прочитает [`decode`] (host уже claimed вызывающим).
pub fn claim(dialect: ValueDialect, host: &Element) {
    match dialect {
        ValueDialect::Edt => {
            if let Some(a) = host.attr(XSI_TYPE) {
                a.claimed.set(true);
                // claim опциональный <value>-лист (узел+текст), как читает decode.
                if let Some(v) = host
                    .children
                    .iter()
                    .find(|c| c.local == "value" && c.prefix.is_empty())
                {
                    v.claim_with_text();
                }
            }
        }
        ValueDialect::Designer => {
            if let Some(a) = host.attr(XSI_NIL) {
                a.claimed.set(true);
            }
            if let Some(a) = host.attr(XSI_TYPE) {
                a.claimed.set(true);
                // Скаляр Designer (string/decimal/dateTime/reference/boolean) — в тексте.
                host.claim_text();
            }
        }
    }
}

// --- ENCODE -----------------------------------------------------------------

/// Восстановить host-элемент из канонического [`ValueSpec`] (byte-exact). Имя/ns хоста —
/// из локуса.
pub fn encode(
    dialect: ValueDialect,
    host_prefix: &str,
    host_local: &str,
    spec: &ValueSpec,
) -> Result<OutElement, String> {
    match dialect {
        ValueDialect::Edt => encode_edt(host_prefix, host_local, spec),
        ValueDialect::Designer => encode_designer(host_prefix, host_local, spec),
    }
}

#[cfg(any())]
mod tests;
