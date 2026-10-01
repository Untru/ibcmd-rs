//! Канонический спек вида объекта `StyleItem` (ЭлементСтиля) — ARCHITECTURE.md §1.4/§1.6.
//! Витнес-вид для двухуровневого стиль-значения ([`ValueKind::StyleValue`]).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для КАЖДОГО
//! формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь — канонический
//! `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта, фреймится каркасом коннектора.
//!
//! Поля (подтверждено сканом SSL, 97 объектов edt+designer) в каноническом DENSE-порядке
//! Designer: `synonym`, `comment`, `type`, `value`. EDT эмитит РАЗРЕЖЁННО (дефолты опущены:
//! пустой `comment`, `type=Color`); Designer — DENSE (`<Comment/>`, `<Type>Color</Type>`).
//!
//! `type` — enum `{Color|Font}` (default `Color`; `Border` — по метамодели, в SSL не встретился).
//! EDT опускает `<type>` для Color (74/97), эмитит `<type>Font</type>` для Font (23/97).
//!
//! `value` — [`ValueKind::StyleValue`] ([`crate::ir::value::StyleValueSpec`]): двухуровневое
//! xsi-типизированное значение шрифта/цвета (`FontValue/FontRef|FontDef`,
//! `ColorValue/ColorRef|ColorDef`). Host ПРИСУТСТВУЕТ ВСЕГДА → поле `required` (без дефолта).
//! Кодировку (EDT nested `<value>` ↔ Designer flat `<Value>`) держит
//! `formats_xml::style_value_codec` — оба формата дают РАВНЫЙ канонический
//! [`crate::ir::value::StyleValueSpec`] (X by construction).
//!
//! `objectBelonging`/`extendedConfigurationObject` — по метамодели, в SSL-корпусе не
//! встречены (0/97 в обоих форматах) → в проекциях НЕ маппятся (дефолт, Absent);
//! оставлены в спеке для полноты метамодели (инвентарь: `todo`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = `[]`; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging` — принадлежность объекта (`Native`/`Adopted`). Default `Native`.
/// В SSL-корпусе не встречен (0/97) — не проецируется форматами; оставлен для метамодели.
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject` — Uuid (расшир. объект конфигурации). Default `""`.
/// В SSL-корпусе не встречен (0/97) — не проецируется форматами; оставлен для метамодели.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `type` — вид стиля (`Color`/`Font`). Default `Color` (EDT опускает; Font эмитит).
pub const F_TYPE: FieldId = FieldId(5);
/// `value` — значение стиля ([`ValueKind::StyleValue`]). Required: host `<value>`/`<Value>`
/// present у всех объектов (это значение, а не дефолт-омиссия).
pub const F_VALUE: FieldId = FieldId(6);

fn localized_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn str_empty(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}
fn enum_field(id: FieldId, name: &'static str, default_token: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, PropertyValue::Enum(Token::new(default_token)))
}

/// Сконструировать [`FieldSpec`] вида `StyleItem` в каноническом DENSE-порядке Designer.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        localized_field(F_SYNONYM, "synonym"),
        str_empty(F_COMMENT, "comment"),
        enum_field(F_OBJECT_BELONGING, "objectBelonging", "Native"),
        str_empty(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),
        enum_field(F_TYPE, "type", "Color"),
        // value — StyleValue-вид, host present всегда → required (без дефолта).
        FieldSpec::required(F_VALUE, "value", ValueKind::StyleValue),
    ]
}

/// Канонический [`EntitySpec`] вида `StyleItem` (кэш на процесс).
pub fn style_item() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "StyleItem",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

/// GUID объекта `StyleItem` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "58848766-36ea-4076-8800-e91eb49590d7";
