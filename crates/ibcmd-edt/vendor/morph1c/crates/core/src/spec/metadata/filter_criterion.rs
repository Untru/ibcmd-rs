//! Канонический спек вида объекта `FilterCriterion` (критерий отбора) —
//! ARCHITECTURE.md §1.0/§1.6, child-objects substrate. Лист-подобный ссылочный вид (как
//! DocumentJournal по «лёгкости»): нет DB-persistence (нумерации/кода/иерархии), но НЕСЁТ
//! `Type` (ОписаниеТипов отбираемого поля), список `Content` (ссылки на реквизиты-члены
//! критерия) и подчинённые коллекции Form/Command. Переиспользует СУЩЕСТВУЮЩИЙ субстрат:
//! `Type`-кодек, `RefList` (Content), FormRef (bare-ref, X по имени+порядку) и Command
//! (= Report.Command по набору полей) — НОВОГО общего субстрата не требует.
//!
//! Поля (Designer DENSE `<Properties>`, сверено корпусом ERP 10 объектов edt+designer):
//! `synonym, comment, type, useStandardCommands, content, defaultForm, auxiliaryForm,
//! listPresentation, extendedListPresentation, explanation`. EDT эмитит РАЗРЕЖЁННО (дефолты
//! опущены); физический порядок EDT совпадает с каноническим спек-порядком (comment
//! дефолтен → опущен между synonym и type), поэтому `field_emit_order` не нужен.
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта; `producedTypes`
//! (List+Manager) — платформенный [`crate::ir::InternalInfo`]-блок, фреймится каркасом
//! коннектора через `PRODUCED_CATEGORIES` (ниже), не движком.
//!
//! Метамодельные поля `objectBelonging`/`extendedConfigurationObject` (EDT-xcore) в
//! корпусе ERP отсутствуют (n=0) и НЕ включены (та же конвенция, что у `w_s_reference`/
//! `document_numerator`): их эмиссия сломала бы Designer DENSE byte-exact (Designer их не
//! несёт). `managerModule`/`dbViewDefs`/`extension`/`suppressObject`/`standardCommands` —
//! платформенно-вычисляемый каркас, корпусом не витнессятся.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — описание типа отбираемого поля (`ОписаниеТипов`). Required: host-элемент
/// `<type>`/`<Type>` присутствует у всех объектов (present всегда).
pub const F_TYPE: FieldId = FieldId(3);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(4);
/// `content` — список ссылок-путей на реквизиты-члены критерия (`MdObject` mult=many).
/// Default = пустой список `[]`.
pub const F_CONTENT: FieldId = FieldId(5);
/// `defaultForm` — form-ref. Default "".
pub const F_DEFAULT_FORM: FieldId = FieldId(6);
/// `auxiliaryForm` — form-ref. Default "".
pub const F_AUXILIARY_FORM: FieldId = FieldId(7);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(8);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(9);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(10);

fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        // Type — обязательное поле (нет дефолта): описание типа присутствует всегда.
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
        FieldSpec::with_default(F_USE_STANDARD_COMMANDS, "useStandardCommands", ValueKind::Bool, PropertyValue::Bool(false)),
        // content — список ссылок (RefList); пустой = дефолт `[]`.
        FieldSpec::with_default(F_CONTENT, "content", ValueKind::List, PropertyValue::List(Vec::new())),
        s(F_DEFAULT_FORM, "defaultForm"),
        s(F_AUXILIARY_FORM, "auxiliaryForm"),
        loc(F_LIST_PRESENTATION, "listPresentation"),
        loc(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc(F_EXPLANATION, "explanation"),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено ERP: Form < Command).
/// Form — BARE-ref коллекция (X по имени+порядку); Command — полный симметричный sub-object.
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Form", child_kind: "FilterCriterion.FormRef" },
    ChildSlot { collection: "Command", child_kind: "FilterCriterion.Command" },
];

/// Канонический [`EntitySpec`] вида `FilterCriterion` (кэш на процесс).
pub fn filter_criterion() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "FilterCriterion",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
//
// FilterCriterion несёт ДВЕ категории — `List` и `Manager`. EDT-порядок:
// `<listType>` затем `<managerType>` (List=0, Manager=1). Designer-порядок InternalInfo
// РАСХОДИТСЯ: Manager затем List (`designer_order`: Manager=0, List=1) — сверено ERP 10/10.
// ============================================================================
/// Категории producedTypes вида `FilterCriterion` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "FilterCriterionList",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "FilterCriterionManager",
        designer_order: 0,
    },
];
