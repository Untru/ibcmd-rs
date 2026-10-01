//! Канонический спек вида объекта `FunctionalOption` (функциональная опция) —
//! ARCHITECTURE.md §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) — идентичность
//! объекта в [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 76 объектов edt+designer):
//! * `Synonym` — локализованный синоним;
//! * `Comment` — свободный текст (EDT опускает дефолт, Designer `<Comment/>`);
//! * `Location` — ссылка-хранилище значения опции (`Constant.X` / `Catalog.X.Attribute.Y`
//!   / `InformationRegister.X.Resource.Y`). Плоская строка-ссылка, present у всех 76 →
//!   required;
//! * `PrivilegedGetMode` — bool привилегированного чтения. В корпусе SSL = `true` у всех
//!   76; default = `false` (дефолт платформы; при `false` опускается). EDT presence-bool,
//!   Designer text-bool;
//! * `Content` — СОСТАВ опции: список ссылок-объектов, на которые она влияет. EDT —
//!   сиблинги `<content>Path</content>`; Designer — `<Content><xr:Object>Path</xr:Object>…`
//!   (object-style). Канонический IR — `List([Str(path)])`; пустой → `[]` (дефолт).
//!
//! Лист-вид: подчинённых коллекций нет (`Content` — СВОЙСТВО-список ссылок, не child).
//! Порядок = DENSE-порядок Designer `<Properties>`: Synonym, Comment, Location,
//! PrivilegedGetMode, Content (EDT эмитит то же, опуская дефолтный comment).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `location` — ссылка-хранилище значения опции (плоская строка). Required (present у всех).
pub const F_LOCATION: FieldId = FieldId(3);
/// `privilegedGetMode` — привилегированное чтение. Default = `false` (опускается).
pub const F_PRIVILEGED_GET_MODE: FieldId = FieldId(4);
/// `content` — состав опции (СПИСОК ссылок-объектов). Default = пустой список.
pub const F_CONTENT: FieldId = FieldId(5);

/// Сконструировать [`FieldSpec`] вида `FunctionalOption` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        // Location — плоская строка-ссылка (required).
        FieldSpec::required(F_LOCATION, "location", ValueKind::Str),
        // PrivilegedGetMode — bool; default false (платформенный дефолт).
        FieldSpec::with_default(
            F_PRIVILEGED_GET_MODE,
            "privilegedGetMode",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // Content — список ссылок-объектов; default = пустой список.
        FieldSpec::with_default(F_CONTENT, "content", ValueKind::List, PropertyValue::List(Vec::new())),
    ]
}

/// Канонический [`EntitySpec`] вида `FunctionalOption` (кэш на процесс).
pub fn functional_option() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "FunctionalOption",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
