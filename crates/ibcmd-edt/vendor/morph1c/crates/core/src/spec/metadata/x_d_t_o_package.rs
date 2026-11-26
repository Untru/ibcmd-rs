//! Канонический спек вида объекта `XDTOPackage` (пакет XDTO — набор описаний XML-типов) —
//! ARCHITECTURE.md §1.4/§1.6, срез S4 (XDTO package sidecar + рекурсивная схема).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write КАЖДОГО
//! XML-формата (EDT `.mdo`, Designer `.xml`) из него — формат хранит лишь ПРОЕКЦИЮ (имя
//! тега/ns) в зеркальном модуле. Здесь — канонический `id`, `value_kind`, `default` и
//! ПОРЯДОК эмиссии (§1.6).
//!
//! # Что здесь (и чего НЕТ)
//! Дескриптор `.mdo`/`.xml` вида XDTOPackage — ТОНКИЙ (сверено корпусом SSL 54/54): несёт
//! `synonym` (все 54), `comment` (дефолт `""` — EDT опускает, Designer эмитит `<Comment/>`)
//! и `namespace` (целевое пространство имён пакета — присутствует у всех). Сама СХЕМА
//! (рекурсивное дерево `objectType`/`valueType`/`property`/…) живёт в ОТДЕЛЬНОМ текстовом
//! XML-СПУТНИКЕ `Package.xdto` (EDT) / `Ext/Package.bin` (Designer), ns
//! `http://v8.1c.ru/8.1/xdto`, root `<package>`. Спутник — НЕ спек-свойство, разбираемое
//! движком: он фреймится каркасом коннектора через XDTO-schema-кодек (`formats_xml::xdto` +
//! sidecar-aware R/X в testkit), аналогично тому как таблица прав Role (`Rights.rights`) или
//! `Module.bsl` фреймятся вне спек-региона. Поэтому в `fields()` его нет.
//!
//! Метамодель-only поля (`objectBelonging`/`extendedConfigurationObject`, 0 вхождений в SSL)
//! НЕ спекнуты — нет свидетеля (§1.0: не спекаем невитнессированное; ср. Role). Если
//! появятся в корпусе — добавляются с witnessed-дефолтом.
//!
//! `name`/`uuid` — идентичность объекта ([`crate::ir::MetadataObject`]), round-trip-ится
//! каркасом, а не [`FieldSpec`] (как у всех видов).
//!
//! # Порядок полей (= порядок эмиссии Designer `<Properties>`, сверено корпусом SSL)
//! `Synonym` → `Comment` → `Namespace`. Канонический IR РАЗРЕЖЕН (§1.1): EDT `.mdo` эмитит
//! лишь non-default; Designer DENSE эмитит и дефолт `<Comment/>`, но движок сводит к тому же
//! РАЗРЕЖЁННОМУ IR → edt==designer (X by construction, §1.6/§3.5).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка для X.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `namespace` — целевое пространство имён пакета XDTO (свободная строка). Присутствует у
/// всех 54 объектов SSL (не дефолт-омиссия), поэтому — обязательное поле.
pub const F_NAMESPACE: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`]'ы вида `XDTOPackage` в каноническом порядке.
///
/// Не `static`: `default`-значения ([`PropertyValue`]) владеют `String`/`Vec` (не
/// `const`-конструируемы). Кэшируется на первый вызов ([`x_d_t_o_package`]).
fn build_fields() -> Vec<FieldSpec> {
    vec![
        // synonym: Localized, default пустой список, маркер сорт-по-lang для X.
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        // comment: Str, default "".
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        // namespace: Str, required (present-empty валидно, присутствует у всех).
        FieldSpec::required(F_NAMESPACE, "namespace", ValueKind::Str),
    ]
}

/// Канонический [`EntitySpec`] вида `XDTOPackage` (кэш на процесс).
///
/// 3 тонких свойства дескриптора в каноническом порядке эмиссии (`name`/`uuid` —
/// идентичность объекта, тут НЕ перечислены; схема пакета — текстовый sidecar, вне спека).
pub fn x_d_t_o_package() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "XDTOPackage",
        // Утечка единожды на процесс → владеемый Vec в `&'static [_]` без unsafe.
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // схема — текстовый sidecar (каркас), не child-коллекция.
    })
}

// Реестр (`crate::spec::registry`) собирается build.rs'ом, который ИМЕНУЕТ
// `x_d_t_o_package::x_d_t_o_package` — отдельной саморегистрации не нужно. Код вида берётся
// из `EntitySpec::entity` (== "XDTOPackage").
