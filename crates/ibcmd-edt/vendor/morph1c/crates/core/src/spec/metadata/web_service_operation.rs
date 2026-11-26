//! Канонический спек ДОЧЕРНЕГО вида `WebService.Operation` — операция Web-сервиса.
//! child-objects substrate, §1.1.
//!
//! **RECURSION-узел:** несёт собственную child-коллекцию `Parameter`
//! (`WebService.Operation.Parameter`) — доказательство рекурсивного субстрата
//! (родитель `WebService` → `Operation` → `Parameter`, два уровня вложенности).
//!
//! Dotted-kind ключ (`"WebService.Operation"`); stem = ASCII snake_case, функция = stem
//! (build.rs). БЕЗ собственного `HARNESS_ENTRY` — покрыт ТРАНЗИТИВНО через корень.
//!
//! Спек-свойства (скан SSL, 143 Operation × 13 WebService, edt+designer). Порядок =
//! DENSE-порядок Designer (`<Properties>`: Name(идентичность), Synonym, Comment,
//! XDTOReturningValueType, Nillable, Transactioned, ProcedureName, DataLockControlMode).
//! EDT — РАЗРЕЖЁННО (опускает дефолтные `comment`/`nillable`/`transactioned`), Designer
//! — DENSE. `xdtoReturningValueType` — xdto-type-ref (пара `name`+`nsUri`), present ВСЕГДА
//! (143/143). Собственные дети `Parameter` — НЕ свойства, а [`ChildSlot`] (рекурсия).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним операции. Default = пустой список; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `xdtoReturningValueType` — XDTO-type-ref возвращаемого значения (пара `name`+`nsUri`).
/// Required (143/143). IR — `List([Str(name), Str(nsUri)])`.
pub const F_XDTO_RETURNING_VALUE_TYPE: FieldId = FieldId(3);
/// `nillable` — допускает ли значение null. Default = `false` (EDT опускает).
pub const F_NILLABLE: FieldId = FieldId(4);
/// `transactioned` — выполнять в транзакции. Default = `false` (EDT ВСЕГДА опускает).
pub const F_TRANSACTIONED: FieldId = FieldId(5);
/// `procedureName` — имя обработчика-процедуры. Required (143/143).
pub const F_PROCEDURE_NAME: FieldId = FieldId(6);
/// `dataLockControlMode` — режим управления блокировкой данных. Default =
/// `Automatic` (EDT omits it; Designer emits it). Confirmed against genuine UH
/// operation pairs and EDT MdProperty 09c14dad-bd23-4d30-9c09-20ca3e5cd9f0.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(7);

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
        FieldSpec::required(F_XDTO_RETURNING_VALUE_TYPE, "xdtoReturningValueType", ValueKind::List),
        FieldSpec::with_default(F_NILLABLE, "nillable", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(F_TRANSACTIONED, "transactioned", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::required(F_PROCEDURE_NAME, "procedureName", ValueKind::Str),
        FieldSpec::with_default(
            F_DATA_LOCK_CONTROL_MODE,
            "dataLockControlMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Automatic")),
        ),
    ]
}

/// Слоты дочерних коллекций `Operation`: единственная — `Parameter` (рекурсия).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Parameter", child_kind: "WebService.Operation.Parameter" }];

/// Канонический [`EntitySpec`] вида `WebService.Operation` (кэш на процесс).
/// Recursion-узел: несёт `Parameter`-коллекцию.
pub fn web_service_operation() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WebService.Operation",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}
