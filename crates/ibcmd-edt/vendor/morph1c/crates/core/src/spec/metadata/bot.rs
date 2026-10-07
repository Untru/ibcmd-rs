//! Канонический спек вида объекта `Bot` (Бот) — ARCHITECTURE.md §1.4/§1.6.
//! Лист-вид (UI/сервис). Один [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит
//! read/write для КАЖДОГО формата (EDT `.mdo`, Designer `.xml`); cf — зеркальная проекция.
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта, фреймится каркасом.
//!
//! Поля (сверено корпусом `coverage/s9_new`, 2 объекта edt+designer+cf) в каноническом
//! DENSE-порядке Designer: `synonym`, `comment`, `predefined`, `picture`. EDT эмитит
//! РАЗРЕЖЁННО (дефолты опущены: `predefined=false`, пустой `picture`); Designer — DENSE
//! (`<Predefined>…</Predefined>`, `<Picture/>`). objectBelonging/extendedConfigurationObject
//! по метамодели существуют, но в этом корпусе НЕ витнессятся (0/2 в обоих форматах и в cf) —
//! в спек НЕ включены (§1.0: не эмитим невитнессенное; auto-derive проецирует КАЖДОЕ поле,
//! поэтому спек = ровно витнессенные поля).
//!
//! `picture` — ссылка на картинку; в корпусе всегда ПУСТА (Designer `<Picture/>`, EDT опускает,
//! cf — константный empty-picture-блок). Смоделирована как пустой `Str` (default `""`):
//! Designer DENSE эмитит `<Picture/>`, EDT sparse опускает. Появится непустая картинка —
//! кодек расширить структурно (PictureRef).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `predefined` — предопределённый бот (bool). Default `false`.
pub const F_PREDEFINED: FieldId = FieldId(3);
/// `picture` — ссылка на картинку (в корпусе всегда пуста). Default `""`.
pub const F_PICTURE: FieldId = FieldId(4);

fn localized_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn str_empty(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}
fn bool_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}

/// Сконструировать [`FieldSpec`] вида `Bot` в каноническом DENSE-порядке Designer.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        localized_field(F_SYNONYM, "synonym"),
        str_empty(F_COMMENT, "comment"),
        bool_field(F_PREDEFINED, "predefined"),
        str_empty(F_PICTURE, "picture"),
    ]
}

/// Канонический [`EntitySpec`] вида `Bot` (кэш на процесс).
pub fn bot() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Bot",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

/// cf class-guid вида `Bot` (группа корня `.cf`; = строка [`crate::spec`]-независимой
/// KIND_TABLE в `formats-cf/configuration_root.rs`). RE-проверено на `coverage/cf/s9_new.cf`.
pub const ENTITY_GUID: &str = "6e6dc072-b7ac-41e7-8f88-278d25b6da2a";
