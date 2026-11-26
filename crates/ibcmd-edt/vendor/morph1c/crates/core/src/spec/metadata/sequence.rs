//! Канонический спек вида объекта `Sequence` (последовательность документов) —
//! ARCHITECTURE.md §1.0/§1.6, child-objects substrate. Реф-несущий вид среднего размера:
//! БЕЗ форм/шаблонов/команд, но НЕСЁТ `documents`/`registerRecords` (MDObjectRef-списки
//! документов-регистраторов и регистров движений) и дочернюю коллекцию `Dimension`
//! (измерения последовательности — СВОЙ набор полей, НЕ `ir_child::dimension_fields`).
//! Переиспользует СУЩЕСТВУЮЩИЙ субстрат: `RefList`/`MdObjectRefList` (documents/
//! registerRecords), `Type`-кодек (Dimension.type) — НОВОГО общего субстрата не требует.
//!
//! Поля (Designer DENSE `<Properties>`, сверено корпусом coverage/s7_pending 6 объектов
//! edt+designer): `synonym, comment, moveBoundaryOnPosting, documents, registerRecords,
//! dataLockControlMode`. EDT эмитит РАЗРЕЖЁННО (дефолты опущены); физический порядок EDT
//! совпадает с каноническим спек-порядком (== порядок полей метамодели xcore Sequence:
//! moveBoundaryOnPosting < documents < registerRecords < dataLockControlMode), поэтому
//! `field_emit_order` не нужен.
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта; `producedTypes`
//! (Record+Manager+RecordSet, метамодель `SequenceTypes`) — платформенный
//! [`crate::ir::InternalInfo`]-блок, фреймится каркасом коннектора через
//! `PRODUCED_CATEGORIES` (ниже), не движком.
//!
//! Метамодельные поля `objectBelonging`/`extendedConfigurationObject` (EDT-xcore) в
//! корпусе покрытия отсутствуют (n=0) и НЕ включены (конвенция `filter_criterion`/
//! `w_s_reference`). `recordSetModule` — сайдкар-модуль (метамодель несёт ссылку, корпус
//! покрытия модулей Sequence НЕ витнессит); `dbViewDefs`/`extension`/`suppressObject`/
//! `additionalIndexes` — платформенно-вычисляемый каркас, корпусом не витнессится.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `moveBoundaryOnPosting` — сдвигать границу при проведении. Default Move (EDT омитит
/// Move, эмитит DontMove — сверено coverage/s7_pending: НеСмещать несёт тег, прочие нет).
pub const F_MOVE_BOUNDARY_ON_POSTING: FieldId = FieldId(3);
/// `documents` — список ссылок на документы-регистраторы (`Document` mult=many,
/// MDObjectRef). Default = пустой список `[]`.
pub const F_DOCUMENTS: FieldId = FieldId(4);
/// `registerRecords` — список ссылок на регистры движений (`BasicRegister` mult=many,
/// MDObjectRef). Default = `[]` (корпус покрытия витнессит ТОЛЬКО пустой:
/// Designer `<RegisterRecords/>`, EDT — омиссия).
pub const F_REGISTER_RECORDS: FieldId = FieldId(5);
/// `dataLockControlMode` — Default Automatic (EDT омитит Automatic; Managed/
/// AutomaticAndManaged витнессятся coverage/s7_pending).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(6);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(
            F_MOVE_BOUNDARY_ON_POSTING,
            "moveBoundaryOnPosting",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Move")),
        ),
        FieldSpec::with_default(F_DOCUMENTS, "documents", ValueKind::List, PropertyValue::List(Vec::new())),
        FieldSpec::with_default(F_REGISTER_RECORDS, "registerRecords", ValueKind::List, PropertyValue::List(Vec::new())),
        FieldSpec::with_default(
            F_DATA_LOCK_CONTROL_MODE,
            "dataLockControlMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Automatic")),
        ),
    ]
}

/// Слоты дочерних коллекций: ЕДИНСТВЕННАЯ коллекция Dimension (метамодель Sequence несёт
/// лишь `dimensions`; форм/шаблонов/команд у вида НЕТ).
const CHILDREN: &[ChildSlot] = &[ChildSlot { collection: "Dimension", child_kind: "Sequence.Dimension" }];

/// Канонический [`EntitySpec`] вида `Sequence` (кэш на процесс).
pub fn sequence() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Sequence",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe).
//
// Sequence несёт ТРИ категории (метамодель `SequenceTypes`): Record, Manager, RecordSet.
// EDT-порядок: `<recordType>`, `<managerType>`, `<recordSetType>`. Designer-порядок
// InternalInfo СОВПАДАЕТ (Record, Manager, RecordSet — сверено coverage/s7_pending 6/6),
// как и cf-порядок body[1..6].
// ============================================================================
/// Категории producedTypes вида `Sequence` в каноническом порядке IR (= EDT = Designer).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Record",
        edt_tag: "recordType",
        designer_category: "Record",
        designer_type_name: "SequenceRecord",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "SequenceManager",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordSet",
        edt_tag: "recordSetType",
        designer_category: "RecordSet",
        designer_type_name: "SequenceRecordSet",
        designer_order: 2,
    },
];
