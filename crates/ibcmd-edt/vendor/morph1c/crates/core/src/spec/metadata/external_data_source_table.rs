//! Канонический спек ДОЧЕРНЕГО вида `ExternalDataSource.Table` (таблица внешнего источника
//! данных) — child-objects substrate, §1.1.
//!
//! ВИТНЕСС-БАЗА: ERP-корпус (.fixtures/ERP) — единственная непустая таблица
//! `BaseBU.ERP_MASTER_SIVANOV_public__accumrg90126` (32 поля), оба диалекта:
//! * EDT: ОТДЕЛЬНЫЙ файл `ExternalDataSources/BaseBU/Tables/<Имя>/<Имя>.mdo`
//!   (`mdclass:Table` корень; в родительском `.mdo` — лишь дотированная ссылка
//!   `<tables>ExternalDataSource.BaseBU.Table.<Имя></tables>`);
//! * Designer: ОТДЕЛЬНЫЙ сайдкар `ExternalDataSources/BaseBU/Tables/<Имя>.xml`
//!   (`MetaDataObject/Table` c InternalInfo+Properties+ChildObjects/Field; в родительском
//!   `.xml` — bare-ссылка `<ChildObjects><Table>Имя</Table></ChildObjects>`).
//!
//! Coverage-корпус несёт ПУСТУЮ коллекцию (прежняя заглушка) — расширение обратносовместимо.
//!
//! ПОРЯДОК ПОЛЕЙ: один канонический порядок обслуживает ОБА диалекта (сверено витнессом:
//! EDT SPARSE-последовательность `parentDataSource < nameInDataSource < tableDataType <
//! keyFields < unfilledParentValue < useStandardCommands < readOnly < editType` и Designer
//! DENSE-последовательность `TableType … DataLockControlMode` строго возрастают по спеку).
//! `parentDataSource` — EDT-only (Designer выражает принадлежность ТОЛЬКО расположением
//! сайдкара на диске — как `Subsystem.parentSubsystem`; designer-чтение синтезирует его в
//! пайплайне, EDT-чтение кросс-сверяет — см. `pipeline::table_ref_read`).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (8 категорий, см. [`PRODUCED_CATEGORIES`]). `objectBelonging`/
//! `extendedConfigurationObject`/`help` (xcore-черновик) в корпусе не витнессированы (n=0)
//! и НЕ включены — их эмиссия сломала бы Designer DENSE byte-exact (конвенция
//! не-generated спеков, как у родителя `external_data_source`).
//!
//! cf-сторона: cf-кодировка EDS-таблиц НЕ витнессирована ни одним оракулом (coverage-cf несёт
//! только пустые коллекции) → cf-writer родителя даёт типизированный отказ на непустых
//! children (`formats_cf::metadata::external_data_source`); witness снимать с erp.cf.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `tableType` — `{Table|Expression}`. Default `Table` (EDT омитит; Designer DENSE).
pub const F_TABLE_TYPE: FieldId = FieldId(3);
/// `parentDataSource` — дотированная ссылка на родителя (`ExternalDataSource.BaseBU`).
/// EDT-ONLY (Designer не несёт: принадлежность = положение сайдкара на диске; designer-чтение
/// синтезирует в пайплайне — зеркало `Subsystem.parentSubsystem`). Default "".
pub const F_PARENT_DATA_SOURCE: FieldId = FieldId(4);
/// `nameInDataSource` — имя таблицы в источнике (`ERP_MASTER_SIVANOV.public._accumrg90126`).
pub const F_NAME_IN_DATA_SOURCE: FieldId = FieldId(5);
/// `expressionInDataSource` — выражение-источник (у Expression-таблиц). Default "".
pub const F_EXPRESSION_IN_DATA_SOURCE: FieldId = FieldId(6);
/// `tableDataType` — `{ObjectData|NonobjectData}`. Default `ObjectData` (witness несёт
/// NonobjectData ПРИСУТСТВУЮЩИМ в EDT — т.е. не-дефолт; дефолт из xcore-метамодели).
pub const F_TABLE_DATA_TYPE: FieldId = FieldId(7);
/// `keyFields` — список дотированных ссылок на ключевые поля (RefList: EDT сиблинги
/// `<keyFields>Путь</keyFields>`, Designer `<KeyFields><xr:Field>Путь</xr:Field></KeyFields>`;
/// путь ПОЛНЫЙ: `ExternalDataSource.<EDS>.Table.<T>.Field.<F>` — witness 2 ключа). Default [].
pub const F_KEY_FIELDS: FieldId = FieldId(8);
/// `presentationField` — ссылка-поле представления. Default "".
pub const F_PRESENTATION_FIELD: FieldId = FieldId(9);
/// `parentField` — ссылка-поле родителя (иерархия). Default "".
pub const F_PARENT_FIELD: FieldId = FieldId(10);
/// `unfilledParentValue` — REQUIRED Value (witness: EDT `xsi:type="core:UndefinedValue"`,
/// Designer `xsi:nil="true"` — оба канонизируются Value-кодеком в Undefined).
pub const F_UNFILLED_PARENT_VALUE: FieldId = FieldId(11);
/// `characteristics` — виды характеристик (witness: пуст; Designer `<Characteristics/>`).
pub const F_CHARACTERISTICS: FieldId = FieldId(12);
/// `useStandardCommands` — Default false (witness: `true` present в ОБОИХ диалектах).
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(13);
/// `quickChoice` — Bool (у ТАБЛИЦЫ — bool, у Field — enum!). Default false.
pub const F_QUICK_CHOICE: FieldId = FieldId(14);
/// `inputByString` — RefList. Default [] (witness: `<InputByString/>`).
pub const F_INPUT_BY_STRING: FieldId = FieldId(15);
/// `createOnInput` — `{Auto|DontUse|Use}`. Default Auto.
pub const F_CREATE_ON_INPUT: FieldId = FieldId(16);
/// `searchStringModeOnInputByString` — `{Begin|AnyPart}`. Default Begin.
pub const F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING: FieldId = FieldId(17);
/// `choiceDataGetModeOnInputByString` — `{Directly|Background}`. Default Directly.
pub const F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING: FieldId = FieldId(18);
/// `choiceHistoryOnInput` — `{Auto|DontUse}`. Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(19);
/// `defaultObjectForm` — ссылка-форма. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(20);
/// `defaultRecordForm` — ссылка-форма. Default "".
pub const F_DEFAULT_RECORD_FORM: FieldId = FieldId(21);
/// `defaultListForm` — ссылка-форма. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(22);
/// `defaultChoiceForm` — ссылка-форма. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(23);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(24);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(25);
/// `recordPresentation` — локализ. Default [].
pub const F_RECORD_PRESENTATION: FieldId = FieldId(26);
/// `extendedRecordPresentation` — локализ. Default [].
pub const F_EXTENDED_RECORD_PRESENTATION: FieldId = FieldId(27);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(28);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(29);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(30);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(31);
/// `readOnly` — Default false (witness: `true` present в обоих).
pub const F_READ_ONLY: FieldId = FieldId(32);
/// `transactionsIsolationLevel` — Default Auto (witness Designer DENSE `Auto`, EDT омитит).
pub const F_TRANSACTIONS_ISOLATION_LEVEL: FieldId = FieldId(33);
/// `dataVersionField` — ссылка-поле версии данных. Default "".
pub const F_DATA_VERSION_FIELD: FieldId = FieldId(34);
/// `editType` — `{InList|InDialog|BothWays}`. Default InList (witness: InDialog present).
pub const F_EDIT_TYPE: FieldId = FieldId(35);
/// `basedOn` — RefList (item-style). Default [] (witness: `<BasedOn/>`).
pub const F_BASED_ON: FieldId = FieldId(36);
/// `dataLockFields` — RefList (field-style). Default [] (witness: `<DataLockFields/>`).
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(37);
/// `dataLockControlMode` — `{Automatic|Managed|AutomaticAndManaged}`. Default Automatic
/// (witness Designer DENSE `Automatic`, EDT омитит).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(38);

fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn b(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn e(id: FieldId, name: &'static str, def: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, PropertyValue::Enum(Token::new(def)))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}
fn list(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::List, PropertyValue::List(Vec::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_TABLE_TYPE, "tableType", "Table"),
        s(F_PARENT_DATA_SOURCE, "parentDataSource"),
        s(F_NAME_IN_DATA_SOURCE, "nameInDataSource"),
        s(F_EXPRESSION_IN_DATA_SOURCE, "expressionInDataSource"),
        e(F_TABLE_DATA_TYPE, "tableDataType", "ObjectData"),
        list(F_KEY_FIELDS, "keyFields"),
        s(F_PRESENTATION_FIELD, "presentationField"),
        s(F_PARENT_FIELD, "parentField"),
        FieldSpec::required(F_UNFILLED_PARENT_VALUE, "unfilledParentValue", ValueKind::Value),
        list(F_CHARACTERISTICS, "characteristics"),
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        b(F_QUICK_CHOICE, "quickChoice"),
        list(F_INPUT_BY_STRING, "inputByString"),
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING, "searchStringModeOnInputByString", "Begin"),
        e(F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING, "choiceDataGetModeOnInputByString", "Directly"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_RECORD_FORM, "defaultRecordForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        loc(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc(F_RECORD_PRESENTATION, "recordPresentation"),
        loc(F_EXTENDED_RECORD_PRESENTATION, "extendedRecordPresentation"),
        loc(F_LIST_PRESENTATION, "listPresentation"),
        loc(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc(F_EXPLANATION, "explanation"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        b(F_READ_ONLY, "readOnly"),
        e(F_TRANSACTIONS_ISOLATION_LEVEL, "transactionsIsolationLevel", "Auto"),
        s(F_DATA_VERSION_FIELD, "dataVersionField"),
        e(F_EDIT_TYPE, "editType", "InList"),
        list(F_BASED_ON, "basedOn"),
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
    ]
}

/// Вложенная коллекция `Field` (EDT inline `<tableFields uuid>`; Designer
/// `<ChildObjects><Field uuid><Properties>`).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Field", child_kind: "ExternalDataSource.Table.Field" }];

/// `&'static EntitySpec` вида `ExternalDataSource.Table` (кэш на процесс).
pub fn external_data_source_table() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ExternalDataSource.Table",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// build.rs подхватывает этот `pub const` по имени БЕЗ правки общих файлов.
// ============================================================================
/// Категории producedTypes вида `ExternalDataSource.Table` (witness ERP BaseBU-таблица,
/// оба диалекта). EDT-порядок тегов: `refType, listType, objectType, managerType,
/// recordManagerType, recordSetType, recordType, recordKeyType`; Designer InternalInfo-порядок:
/// `Manager, Object, Ref, List, Record, RecordSet, RecordKey, RecordManager`; designer-имена:
/// `ExternalDataSourceTable<Категория>.<EDS>.<Таблица>` (префикс-цепочка родителя —
/// name_prefix-механизм каркаса).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "ExternalDataSourceTableRef",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "ExternalDataSourceTableList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "ExternalDataSourceTableObject",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ExternalDataSourceTableManager",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordManager",
        edt_tag: "recordManagerType",
        designer_category: "RecordManager",
        designer_type_name: "ExternalDataSourceTableRecordManager",
        designer_order: 7,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordSet",
        edt_tag: "recordSetType",
        designer_category: "RecordSet",
        designer_type_name: "ExternalDataSourceTableRecordSet",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "Record",
        edt_tag: "recordType",
        designer_category: "Record",
        designer_type_name: "ExternalDataSourceTableRecord",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordKey",
        edt_tag: "recordKeyType",
        designer_category: "RecordKey",
        designer_type_name: "ExternalDataSourceTableRecordKey",
        designer_order: 6,
    },
];
