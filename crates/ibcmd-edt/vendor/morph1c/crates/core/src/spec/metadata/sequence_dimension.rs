//! Канонический спек ДОЧЕРНЕГО вида `Sequence.Dimension` (измерение последовательности)
//! — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно через Sequence).
//!
//! НЕ переиспользует [`crate::spec::ir_child::dimension_fields`]: у измерения
//! последовательности СВОЙ (узкий) набор полей — метамодель xcore `SequenceDimension`
//! несёт лишь `type` + `documentMap`/`registerRecordsMap` (соответствия реквизитам
//! документов/регистров), БЕЗ UI/заполнения/индексирования регистровых измерений.
//!
//! Поля (Designer DENSE `<Properties>`, сверено coverage/s7_pending 6 Dimension):
//! `synonym, comment, type, documentMap, registerRecordsMap`. EDT эмитит РАЗРЕЖЁННО;
//! физический порядок = спек-порядку. `documentMap`/`registerRecordsMap` витнессятся
//! ТОЛЬКО пустыми (Designer `<DocumentMap/>`/`<RegisterRecordsMap/>`, EDT — омиссия).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — описание типа измерения (`ОписаниеТипов`). Required (present всегда).
pub const F_TYPE: FieldId = FieldId(3);
/// `documentMap` — соответствия реквизитам документов (`BasicFeature` mult=many).
/// Default `[]` (корпус витнессит ТОЛЬКО пустой).
pub const F_DOCUMENT_MAP: FieldId = FieldId(4);
/// `registerRecordsMap` — соответствия измерениям регистров (`BasicFeature` mult=many).
/// Default `[]` (корпус витнессит ТОЛЬКО пустой).
pub const F_REGISTER_RECORDS_MAP: FieldId = FieldId(5);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
        FieldSpec::with_default(F_DOCUMENT_MAP, "documentMap", ValueKind::List, PropertyValue::List(Vec::new())),
        FieldSpec::with_default(
            F_REGISTER_RECORDS_MAP,
            "registerRecordsMap",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
    ]
}

/// `&'static EntitySpec` вида `Sequence.Dimension` (кэш на процесс).
pub fn sequence_dimension() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Sequence.Dimension",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
