//! Канонический спек вида объекта `Style` (Стиль) — ARCHITECTURE.md §1.4/§1.6.
//! Контейнер элементов стиля (`StyleItem`); сам объект — чистый лист-присутствие:
//! идентичность + локализованный синоним + комментарий (в coverage-корпусе — объект-
//! заглушка `Стиль_Присутствие`, без под-элементов).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для КАЖДОГО
//! формата (EDT `.mdo`, Designer `.xml`, cf brace). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта, фреймится каркасом коннектора.
//!
//! Поля (по метамодели `Style`, guid=3e5404af-6ef8-4c73-ad11-91bd2dfac4c8; набор совпадает с
//! `_generated/style.rs`): `synonym`, `comment`, `objectBelonging`, `extendedConfigurationObject`.
//! `objectBelonging`/`extendedConfigurationObject` — по метамодели, в coverage-корпусе не
//! встречены (объект-заглушка несёт только synonym/comment) → в проекциях НЕ маппятся
//! (дефолт, Absent); оставлены в спеке для полноты метамодели (зеркалит `style_item`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = `[]`; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging` — принадлежность объекта (`Native`/`Adopted`). Default `Native`.
/// В coverage-корпусе не встречен — не проецируется форматами; оставлен для метамодели.
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject` — Uuid (расшир. объект конфигурации). Default `""`.
/// В coverage-корпусе не встречен — не проецируется форматами; оставлен для метамодели.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);

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

/// Сконструировать [`FieldSpec`] вида `Style` в каноническом DENSE-порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        localized_field(F_SYNONYM, "synonym"),
        str_empty(F_COMMENT, "comment"),
        enum_field(F_OBJECT_BELONGING, "objectBelonging", "Native"),
        str_empty(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),
    ]
}

/// Канонический [`EntitySpec`] вида `Style` (кэш на процесс).
pub fn style() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Style",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет (StyleItem'ы — отдельный вид).
    })
}

/// GUID объекта `Style` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "3e5404af-6ef8-4c73-ad11-91bd2dfac4c8";
