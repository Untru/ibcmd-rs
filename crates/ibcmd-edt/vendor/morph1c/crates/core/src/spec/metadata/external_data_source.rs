//! Канонический спек вида объекта `ExternalDataSource` (Внешний источник данных) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate. guid=5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5.
//!
//! СТРУКТУРНЫЙ вид: корневые свойства + дочерняя коллекция `Table` (в coverage-корпусе
//! ПУСТА — `<ChildObjects/>`; её содержимое НЕ витнессится, потому лист-child-спек без полей).
//! Родитель сам несёт `producedTypes`/`InternalInfo` — ТРИ категории `Manager`/`TablesManager`/
//! `CubesManager` (R2 data-drive через per-kind [`PRODUCED_CATEGORIES`]).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта; `producedTypes`/`InternalInfo`
//! — платформенный каркас (3 категории, см. `produced_types`).
//!
//! Метамодельные `objectBelonging`/`extendedConfigurationObject` (EDT-xcore) в coverage-корпусе
//! отсутствуют (n=0) и НЕ включены (конвенция не-generated спеков, как у `integration_service`/
//! `document_numerator`): их эмиссия сломала бы Designer DENSE byte-exact (Designer их не несёт).
//! Появится корпус с ними — расширить спек.
//!
//! Поля (Designer DENSE-порядок `<Properties>`, сверено coverage-корпусом, 3 объекта edt+designer;
//! Designer DENSE — все present всегда, EDT SPARSE — дефолты опущены):
//! * `synonym` — локализованный синоним (default []);
//! * `comment` — свободный текст (default `""`);
//! * `dataLockControlMode` — режим управления блокировкой `{Automatic|Managed|
//!   AutomaticAndManaged}` (default `Automatic` — EDT омитит `Automatic`, сверено: объект
//!   `_Автоматический` БЕЗ `<dataLockControlMode>` в EDT ↔ Designer DENSE `Automatic`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = []; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `dataLockControlMode` — `{Automatic|Managed|AutomaticAndManaged}`. Default = `Automatic`
/// (EDT SPARSE омитит дефолт; Designer DENSE эмитит; cf — EnumIndex body-слот 0/1/2).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] вида `ExternalDataSource` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(
            F_DATA_LOCK_CONTROL_MODE,
            "dataLockControlMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Automatic")),
        ),
    ]
}

/// Дочерняя коллекция `Table` (в coverage-корпусе ПУСТА). Слот нужен, чтобы каркас
/// claim'ил/эмитил Designer-обёртку `<ChildObjects/>` (без слота она осталась бы
/// невостребованной → §1.0-ошибка тотальности на чтении).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Table", child_kind: "ExternalDataSource.Table" }];

/// Канонический [`EntitySpec`] вида `ExternalDataSource` (кэш на процесс).
pub fn external_data_source() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ExternalDataSource",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `ExternalDataSource` (идентичность в `.cf`; = `@MdClass ^id` и class-guid
/// группы корня — RE-witnessed на coverage/s5-оракуле: группа `5274d9fc`, count 3 в блоке B8).
pub const ENTITY_GUID: &str = "5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5";

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `ExternalDataSource` в каноническом порядке IR (= EDT):
/// `Manager`/`TablesManager`/`CubesManager` (сверено coverage-корпусом: EDT
/// `<managerType>`/`<tablesManagerType>`/`<cubesManagerType>`; Designer
/// `ExternalDataSourceManager.<Obj>`/`…TablesManager.<Obj>`/`…CubesManager.<Obj>`).
/// КРИТИЧНО для сборки: `harvest_produced_types` (assemble.rs) без них не зарегистрирует
/// type-id GUID'ы объекта в реестр.
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ExternalDataSourceManager",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "TablesManager",
        edt_tag: "tablesManagerType",
        designer_category: "TablesManager",
        designer_type_name: "ExternalDataSourceTablesManager",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "CubesManager",
        edt_tag: "cubesManagerType",
        designer_category: "CubesManager",
        designer_type_name: "ExternalDataSourceCubesManager",
        designer_order: 2,
    },
];
