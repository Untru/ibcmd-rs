//! Канонический спек вида объекта `FunctionalOptionsParameter` (параметр функциональных
//! опций) — ARCHITECTURE.md §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок выводит
//! read/write для КАЖДОГО формата (EDT `.mdo`, Designer `.xml`); формат хранит лишь
//! ПРОЕКЦИЮ; здесь — канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК.
//!
//! ВНЕ спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) — идентичность
//! объекта в [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 4 объекта edt+designer):
//! * `Synonym` — локализованный синоним (4/4);
//! * `Comment` — свободный текст (0/4 непустых; EDT опускает дефолт, Designer `<Comment/>`);
//! * `Use` — ссылка-измерение (`InformationRegister.X.Dimension.Y`). СПИСОК ссылок (в
//!   корпусе ровно одна; default = пустой список → опускается). EDT — сиблинг `<use>Path
//!   </use>`; Designer — контейнер `<Use><xr:Item xsi:type="xr:MDObjectRef">Path</xr:Item>
//!   </Use>` (item-style). Канонический IR — `List([Str(path)])`.
//!
//! Лист-вид: подчинённых коллекций нет. Порядок = DENSE-порядок Designer `<Properties>`:
//! Synonym, Comment, Use (EDT эмитит то же, опуская дефолтный comment).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `use` — ссылка-измерение (СПИСОК ссылок-путей). Default = пустой список (опускается).
pub const F_USE: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] вида `FunctionalOptionsParameter` в каноническом порядке.
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
        // Use — список ссылок (в корпусе single); default = пустой список.
        FieldSpec::with_default(F_USE, "use", ValueKind::List, PropertyValue::List(Vec::new())),
    ]
}

/// Канонический [`EntitySpec`] вида `FunctionalOptionsParameter` (кэш на процесс).
pub fn functional_options_parameter() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "FunctionalOptionsParameter",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
