//! Канонический спек вида объекта `CommonTemplate` (общий макет) — ARCHITECTURE.md
//! §1.4/§1.6. Зеркало по структуре `common_module.rs`:
//! дескриптор-лист без дочерних объектов и без новых value-kind'ов (str/localized/enum).
//!
//! ОДИН [`EntitySpec`] на вид: из него движок ([`crate::engine`]) выводит read/write
//! для КАЖДОГО формата (EDT `.mdo`, Designer `.xml`, далее `.cf`/СУБД). Формат хранит
//! лишь ПРОЕКЦИЮ (имя тега / slot) в зеркальном модуле — здесь только канонический
//! `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ этого спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) —
//! это ИДЕНТИЧНОСТЬ объекта в [`crate::ir::MetadataObject`], round-trip-ится
//! каркасом объекта (ридером формата), а не [`FieldSpec`]. ТЕЛО макета
//! (`Ext/Template.*`) — отдельный артефакт/слайс (Blob), не входит в дескриптор.
//!
//! Порядок 3 полей = канонический порядок эмиссии свойств (см. корпус SSL). Канонический
//! IR РАЗРЕЖЕН: значения, равные `default`, в bag НЕ хранятся (§1.1).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; нормализация
/// сортирует пары по коду языка (детерминизм, §1.6).
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `templateType` — тип макета (enum). Default = `SpreadsheetDocument` (омитнутое в EDT
/// значение, эксплицитно эмитимое Designer'ом — подтверждено по корпусу SSL).
pub const F_TEMPLATE_TYPE: FieldId = FieldId(3);

/// Канонический литерал `templateType = SpreadsheetDocument` (default).
pub const TEMPLATE_TYPE_SPREADSHEET_DOCUMENT: &str = "SpreadsheetDocument";

/// Канонический литерал `templateType = GeographicalSchema` (геосхема). Тело макета несёт
/// СТРУКТУРНО РАЗНЫЕ диалекты в EDT (`Template.geos`, g5) и Designer/cf (`Ext/Template.xml`,
/// extrnprops) — трансформация `formats_xml::geoschema`, pipeline `geos_read`. Канон IR-тело
/// (`Template::body`) — extrnprops-байты (§1.6).
pub const TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA: &str = "GeographicalSchema";

/// Сконструировать 3 [`FieldSpec`] вида `CommonTemplate` в каноническом порядке.
///
/// Не `static`, т.к. `default`-значения ([`PropertyValue`]) владеют `String`/`Vec`
/// (не `const`-конструируемы). Кэшируется на первый вызов ([`common_template`]).
fn build_fields() -> Vec<FieldSpec> {
    vec![
        // synonym: Localized, default пустой список, сорт по lang.
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        // comment: Str, default "".
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        // templateType: Enum, default SpreadsheetDocument.
        FieldSpec::with_default(
            F_TEMPLATE_TYPE,
            "templateType",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(TEMPLATE_TYPE_SPREADSHEET_DOCUMENT)),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonTemplate` (кэш на процесс).
///
/// 3 свойства в каноническом порядке эмиссии (`name`/`uuid` — идентичность объекта,
/// тут НЕ перечислены). Один экземпляр на процесс — `EntitySpec.fields` — `&'static`.
pub fn common_template() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonTemplate",
        // Утечка единожды на процесс — превращает владеемый Vec в `&'static [_]`,
        // которого требует EntitySpec, без unsafe и без правки общего типа спека.
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

// Реестр (`crate::spec::registry`) собирается build.rs'ом, который ИМЕНУЕТ
// `common_template::common_template` — отдельной саморегистрации тут не нужно.
// Канонический код вида берётся из `EntitySpec::entity` (== "CommonTemplate").
