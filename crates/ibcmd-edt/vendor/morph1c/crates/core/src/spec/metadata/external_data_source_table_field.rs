//! Канонический спек ДОЧЕРНЕГО вида `ExternalDataSource.Table.Field` (поле таблицы внешнего
//! источника данных) — child-objects substrate, §1.1.
//!
//! ВИТНЕСС-БАЗА: ERP-корпус — 32 поля таблицы `BaseBU.ERP_MASTER_SIVANOV_public__accumrg90126`
//! в обоих диалектах (EDT inline `<tableFields uuid>` в Table-`.mdo`; Designer
//! `<ChildObjects><Field uuid><Properties>` в Table-сайдкаре). Designer DENSE-порядок ==
//! канонический порядок спека 1:1 (32/32; сверено полным дампом обоих файлов):
//! `Synonym, Comment, Type, PasswordMode, Format, EditFormat, ToolTip, MarkNegatives, Mask,
//! MultiLine, ExtendedEdit, MinValue, MaxValue, FillFromFillingValue, FillValue, FillChecking,
//! ChoiceParameterLinks, ChoiceParameters, QuickChoice, CreateOnInput, ChoiceHistoryOnInput,
//! ChoiceForm, NameInDataSource, ReadOnly, AllowNull`. EDT SPARSE несёт: `synonym, type,
//! minValue, maxValue, fillValue, nameInDataSource, [allowNull]` — строго возрастает по спеку.
//!
//! Дефолты — из xcore-метамодели (`_generated/field.rs` черновик), кросс-сверены DENSE-значениями
//! witness'а (все не-present-в-EDT поля несут в Designer РОВНО дефолт: `PasswordMode=false`,
//! `FillChecking=DontCheck`, `QuickChoice=Auto` (у Field — ENUM, в отличие от bool у Table!),
//! `CreateOnInput=Auto`, `ChoiceHistoryOnInput=Auto`, `ReadOnly=false`, пустые
//! `<ChoiceParameterLinks/>`/`<ChoiceParameters/>`). `objectBelonging`/
//! `extendedConfigurationObject` (xcore) не витнессированы (n=0) → не включены.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность. Собственных producedTypes/детей у Field нет.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — REQUIRED Type (witness: `String(255)` и пр.; Type-кодек обоих диалектов).
pub const F_TYPE: FieldId = FieldId(3);
/// `passwordMode` — Default false.
pub const F_PASSWORD_MODE: FieldId = FieldId(4);
/// `format` — локализ. Default [].
pub const F_FORMAT: FieldId = FieldId(5);
/// `editFormat` — локализ. Default [].
pub const F_EDIT_FORMAT: FieldId = FieldId(6);
/// `toolTip` — локализ. Default [].
pub const F_TOOL_TIP: FieldId = FieldId(7);
/// `markNegatives` — Default false.
pub const F_MARK_NEGATIVES: FieldId = FieldId(8);
/// `mask` — Default "".
pub const F_MASK: FieldId = FieldId(9);
/// `multiLine` — Default false.
pub const F_MULTI_LINE: FieldId = FieldId(10);
/// `extendedEdit` — Default false.
pub const F_EXTENDED_EDIT: FieldId = FieldId(11);
/// `minValue` — REQUIRED Value (witness: EDT `core:UndefinedValue` ↔ Designer `xsi:nil`).
pub const F_MIN_VALUE: FieldId = FieldId(12);
/// `maxValue` — REQUIRED Value (как minValue).
pub const F_MAX_VALUE: FieldId = FieldId(13);
/// `fillFromFillingValue` — Default false.
pub const F_FILL_FROM_FILLING_VALUE: FieldId = FieldId(14);
/// `fillValue` — REQUIRED Value (witness: EDT `core:StringValue` пустой ↔ Designer
/// `xsi:type="xs:string"` пустой).
pub const F_FILL_VALUE: FieldId = FieldId(15);
/// `fillChecking` — `{DontCheck|ShowError}`. Default DontCheck.
pub const F_FILL_CHECKING: FieldId = FieldId(16);
/// `choiceParameterLinks` — Default [] (witness: пуст, `<ChoiceParameterLinks/>`).
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(17);
/// `choiceParameters` — Default [] (witness: пуст, `<ChoiceParameters/>`).
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(18);
/// `quickChoice` — `{Auto|Use|DontUse}` (ENUM — в отличие от bool у Table). Default Auto.
pub const F_QUICK_CHOICE: FieldId = FieldId(19);
/// `createOnInput` — `{Auto|DontUse|Use}`. Default Auto.
pub const F_CREATE_ON_INPUT: FieldId = FieldId(20);
/// `choiceHistoryOnInput` — `{Auto|DontUse}`. Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(21);
/// `choiceForm` — ссылка-форма. Default "".
pub const F_CHOICE_FORM: FieldId = FieldId(22);
/// `nameInDataSource` — имя поля в источнике (witness: `Period`, `'Fld90127'` …). Default "".
pub const F_NAME_IN_DATA_SOURCE: FieldId = FieldId(23);
/// `readOnly` — Default false.
pub const F_READ_ONLY: FieldId = FieldId(24);
/// `allowNull` — Default false (witness: `true` present в EDT у 30/32 полей).
pub const F_ALLOW_NULL: FieldId = FieldId(25);

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
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
        b(F_PASSWORD_MODE, "passwordMode"),
        loc(F_FORMAT, "format"),
        loc(F_EDIT_FORMAT, "editFormat"),
        loc(F_TOOL_TIP, "toolTip"),
        b(F_MARK_NEGATIVES, "markNegatives"),
        s(F_MASK, "mask"),
        b(F_MULTI_LINE, "multiLine"),
        b(F_EXTENDED_EDIT, "extendedEdit"),
        FieldSpec::required(F_MIN_VALUE, "minValue", ValueKind::Value),
        FieldSpec::required(F_MAX_VALUE, "maxValue", ValueKind::Value),
        b(F_FILL_FROM_FILLING_VALUE, "fillFromFillingValue"),
        FieldSpec::required(F_FILL_VALUE, "fillValue", ValueKind::Value),
        e(F_FILL_CHECKING, "fillChecking", "DontCheck"),
        list(F_CHOICE_PARAMETER_LINKS, "choiceParameterLinks"),
        list(F_CHOICE_PARAMETERS, "choiceParameters"),
        e(F_QUICK_CHOICE, "quickChoice", "Auto"),
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        s(F_CHOICE_FORM, "choiceForm"),
        s(F_NAME_IN_DATA_SOURCE, "nameInDataSource"),
        b(F_READ_ONLY, "readOnly"),
        b(F_ALLOW_NULL, "allowNull"),
    ]
}

/// `&'static EntitySpec` вида `ExternalDataSource.Table.Field` (кэш на процесс). Лист-вид.
pub fn external_data_source_table_field() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ExternalDataSource.Table.Field",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
