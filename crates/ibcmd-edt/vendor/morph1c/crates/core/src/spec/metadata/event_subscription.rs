//! Канонический спек вида объекта `EventSubscription` (подписка на событие) —
//! ARCHITECTURE.md §1.4/§1.6.
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) — идентичность
//! объекта в [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 91 объект edt+designer):
//! * `Synonym` — локализованный синоним (91/91);
//! * `Comment` — свободный текст (8/91 непустых; EDT опускает дефолт, Designer `<Comment/>`);
//! * `Source` — `TypeSet`-несущее `ОписаниеТипов` (источник события: НАБОР прикладных
//!   типов-объектов). Required (host `<source>`/`<Source>` present у всех 91). Designer
//!   хостит BARE ref-kind / `DefinedType.Имя` как `<v8:TypeSet>`, концретный `Kind.Имя` —
//!   как `<v8:Type>` (правило [`formats_xml::type_codec`], проверено по корпусу);
//! * `Event` — перечислимый литерал события (`BeforeWrite`/`OnWrite`/`BeforeDelete`/…,
//!   16 литералов). Required. Литерал round-trip-ится byte-exact (X by construction);
//! * `Handler` — обработчик: плоская ссылка-строка `CommonModule.X.Y`. Required.
//!
//! Лист-вид: подчинённых коллекций нет (`Source` — СВОЙСТВО типа, не child-коллекция).
//! Порядок = DENSE-порядок Designer `<Properties>`: Synonym, Comment, Source, Event,
//! Handler (EDT эмитит то же, опуская дефолтный `comment`).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `source` — источник события (`ОписаниеТипов`, несёт `TypeSet`-компоненты). Required:
/// host-элемент `<source>`/`<Source>` присутствует у всех объектов.
pub const F_SOURCE: FieldId = FieldId(3);
/// `event` — литерал события (перечислимый). Required (всегда present).
pub const F_EVENT: FieldId = FieldId(4);
/// `handler` — обработчик (плоская ссылка-строка). Required (всегда present).
pub const F_HANDLER: FieldId = FieldId(5);

/// Сконструировать [`FieldSpec`] вида `EventSubscription` в каноническом порядке.
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
        // Source — обязательное поле (нет дефолта): описание типов источника present всегда.
        FieldSpec::required(F_SOURCE, "source", ValueKind::Type),
        // Event — перечислимый литерал (Token round-trip'ится byte-exact, X by construction).
        FieldSpec::required(F_EVENT, "event", ValueKind::Enum),
        // Handler — плоская строка-ссылка (`CommonModule.X.Y`); кодек PlainText.
        FieldSpec::required(F_HANDLER, "handler", ValueKind::Str),
    ]
}

/// Канонический [`EntitySpec`] вида `EventSubscription` (кэш на процесс).
pub fn event_subscription() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "EventSubscription",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
