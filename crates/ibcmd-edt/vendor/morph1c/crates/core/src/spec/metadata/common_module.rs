//! Канонический спек вида объекта `CommonModule` (общий модуль) — ARCHITECTURE.md
//! §1.4/§1.6.
//!
//! ОДИН [`EntitySpec`] на вид: из него движок ([`crate::engine`]) выводит read/write
//! для КАЖДОГО формата (EDT `.mdo`, Designer `.xml`, далее `.cf`/СУБД). Формат хранит
//! лишь ПРОЕКЦИЮ (имя тега / slot) в зеркальном модуле — здесь только канонический
//! `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ этого спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) —
//! это ИДЕНТИЧНОСТЬ объекта в [`crate::ir::MetadataObject`], round-trip-ится
//! каркасом объекта (ридером формата в F2b-2), а не [`FieldSpec`]. Тело модуля
//! `Module.bsl` — отдельный артефакт/слайс, не входит в дескриптор.
//!
//! Порядок 10 полей = канонический порядок эмиссии свойств (см. бриф). Канонический
//! IR РАЗРЕЖЕН: значения, равные `default`, в bag НЕ хранятся (§1.1).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; нормализация
/// сортирует пары по коду языка (детерминизм, §1.6).
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `global` — глобальный. Default = `false`.
pub const F_GLOBAL: FieldId = FieldId(3);
/// `clientManagedApplication` — клиент (управляемое приложение). Default = `false`.
pub const F_CLIENT_MANAGED_APPLICATION: FieldId = FieldId(4);
/// `server` — сервер. Default = `false`.
pub const F_SERVER: FieldId = FieldId(5);
/// `externalConnection` — внешнее соединение. Default = `false`.
pub const F_EXTERNAL_CONNECTION: FieldId = FieldId(6);
/// `clientOrdinaryApplication` — клиент (обычное приложение). Default = `false`.
pub const F_CLIENT_ORDINARY_APPLICATION: FieldId = FieldId(7);
/// `serverCall` — вызов сервера. Default = `false`.
pub const F_SERVER_CALL: FieldId = FieldId(8);
/// `privileged` — привилегированный. Default = `false`.
pub const F_PRIVILEGED: FieldId = FieldId(9);
/// `returnValuesReuse` — повторное использование возвращаемых значений (enum).
/// Default = `DontUse`; канонические литералы DontUse/DuringRequest/DuringSession.
pub const F_RETURN_VALUES_REUSE: FieldId = FieldId(10);

/// Канонический литерал `returnValuesReuse = DontUse` (default).
pub const RETURN_VALUES_REUSE_DONT_USE: &str = "DontUse";
/// Канонический литерал `returnValuesReuse = DuringRequest`.
pub const RETURN_VALUES_REUSE_DURING_REQUEST: &str = "DuringRequest";
/// Канонический литерал `returnValuesReuse = DuringSession`.
pub const RETURN_VALUES_REUSE_DURING_SESSION: &str = "DuringSession";

/// Сконструировать 10 [`FieldSpec`] вида `CommonModule` в каноническом порядке.
///
/// Не `static`, т.к. `default`-значения ([`PropertyValue`]) владеют `String`/`Vec`
/// (не `const`-конструируемы). Кэшируется на первый вызов ([`common_module`]).
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
        // 7 bool-флагов, default false.
        FieldSpec::with_default(F_GLOBAL, "global", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(
            F_CLIENT_MANAGED_APPLICATION,
            "clientManagedApplication",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(F_SERVER, "server", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(
            F_EXTERNAL_CONNECTION,
            "externalConnection",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_CLIENT_ORDINARY_APPLICATION,
            "clientOrdinaryApplication",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(F_SERVER_CALL, "serverCall", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(F_PRIVILEGED, "privileged", ValueKind::Bool, PropertyValue::Bool(false)),
        // returnValuesReuse: Enum, default DontUse.
        FieldSpec::with_default(
            F_RETURN_VALUES_REUSE,
            "returnValuesReuse",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(RETURN_VALUES_REUSE_DONT_USE)),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonModule` (кэш на процесс).
///
/// 10 свойств в каноническом порядке эмиссии (`name`/`uuid` — идентичность объекта,
/// тут НЕ перечислены). Один экземпляр на процесс — `EntitySpec.fields` — `&'static`.
pub fn common_module() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonModule",
        // Утечка единожды на процесс — превращает владеемый Vec в `&'static [_]`,
        // которого требует EntitySpec, без unsafe и без правки общего типа спека.
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

// Реестр (`crate::spec::registry`) собирается build.rs'ом, который ИМЕНУЕТ
// `common_module::common_module` — отдельной саморегистрации тут не нужно. Канонический
// код вида берётся из `EntitySpec::entity` (== "CommonModule"), источник истины один.
