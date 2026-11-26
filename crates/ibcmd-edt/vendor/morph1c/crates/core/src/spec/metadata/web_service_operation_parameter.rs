//! Канонический спек ДОЧЕРНЕГО вида `WebService.Operation.Parameter` (ЛИСТ) — параметр
//! операции Web-сервиса. child-objects substrate, §1.1.
//!
//! Лист рекурсии (родитель `WebService` → `Operation` → `Parameter`): собственных
//! child-коллекций НЕТ. Dotted-kind ключ (`"WebService.Operation.Parameter"`); stem =
//! ASCII snake_case, функция = stem (build.rs). БЕЗ `HARNESS_ENTRY` — покрыт транзитивно.
//!
//! Спек-свойства (скан SSL, 392 Parameter, edt+designer). Порядок = DENSE-порядок
//! Designer (`<Properties>`: Name(идентичность), Synonym, Comment, XDTOValueType,
//! Nillable, TransferDirection). EDT — РАЗРЕЖЁННО (опускает дефолтные `comment`/`nillable`/
//! `transferDirection`), Designer — DENSE. `xdtoValueType` — xdto-type-ref (пара
//! `name`+`nsUri`), present ВСЕГДА (392/392).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним параметра. Default = пустой список; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `xdtoValueType` — XDTO-type-ref типа параметра (пара `name`+`nsUri`). Required
/// (392/392). IR — `List([Str(name), Str(nsUri)])`.
pub const F_XDTO_VALUE_TYPE: FieldId = FieldId(3);
/// `nillable` — допускает ли значение null. Default = `false` (EDT опускает).
pub const F_NILLABLE: FieldId = FieldId(4);
/// `transferDirection` — направление передачи (`In`/`Out`/`InOut`). Default = `In`
/// (EDT опускает; Designer DENSE эмитит).
pub const F_TRANSFER_DIRECTION: FieldId = FieldId(5);

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
        FieldSpec::required(F_XDTO_VALUE_TYPE, "xdtoValueType", ValueKind::List),
        FieldSpec::with_default(F_NILLABLE, "nillable", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(
            F_TRANSFER_DIRECTION,
            "transferDirection",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("In")),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `WebService.Operation.Parameter` (лист).
pub fn web_service_operation_parameter() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WebService.Operation.Parameter",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
