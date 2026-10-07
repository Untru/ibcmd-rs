//! Канонический спек вида объекта `CommonAttribute` (общий реквизит) — ARCHITECTURE.md
//! §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок выводит read/write для КАЖДОГО формата
//! (EDT `.mdo`, Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь — канонический `id`,
//! `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6). Порядок = DENSE-порядок
//! Designer `<Properties>` (сверено 7 объектов SSL, edt+designer).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта.
//!
//! Метамодельные поля `objectBelonging`/`extendedConfigurationObject` (EDT-xcore) в
//! корпусе SSL отсутствуют (n=0) и НЕ включены: их эмиссия ломала бы Designer DENSE
//! byte-exact (Designer их не несёт), что совпадает с конвенцией не-generated спеков.
//! Если появится корпус с этими полями — расширить спек.
//!
//! Поля (Designer DENSE — все present всегда; EDT SPARSE — дефолты опущены):
//! * `synonym` — локализованный синоним (default `[]`);
//! * `comment` — свободный текст (default `""`);
//! * `type` — ОписаниеТипов (required, present всегда);
//! * `passwordMode`/`markNegatives`/`multiLine`/`extendedEdit`/`fillFromFillingValue` —
//!   bool'ы (default `false`);
//! * `format`/`editFormat`/`toolTip` — локализованные (default `[]`);
//! * `mask`/`choiceForm`/`dataSeparationValue`/`dataSeparationUse`/`conditionalSeparation`
//!   — свободный текст (default `""`);
//! * `minValue`/`maxValue`/`fillValue` — nullable-скаляр `Value` (required, present всегда);
//! * `fillChecking` `{DontCheck|ShowError}`, `choiceFoldersAndItems` `{Items|Folders|
//!   FoldersAndItems}`, `quickChoice`/`createOnInput`/`choiceHistoryOnInput` `{Auto|Use|
//!   DontUse}`, `dataHistory` `{DontUse|Use}`, `autoUse` `{Use|DontUse}`, `dataSeparation`
//!   `{Separate|DontUse}`, `separatedDataUse` `{Independently|IndependentlyAndSimultaneously}`,
//!   `usersSeparation`/`authenticationSeparation`/`configurationExtensionsSeparation`
//!   `{Separate|DontUse}`, `indexing` `{DontIndex|Index|IndexWithAdditionalOrder}`,
//!   `fullTextSearch` `{DontUse|Use}` — enum'ы;
//! * `choiceParameterLinks`/`choiceParameters` — списки (default `[]`);
//! * `linkByType` — связь по типу (`Str`, default `""`);
//! * `content` — СОСТАВ общего реквизита: `List` пар `(metadata-ref, use)` (non-empty во
//!   всех объектах корпуса — load-bearing). Кодек [`formats_xml::common_attribute_content`].
//!
//! Лист-вид: подчинённых коллекций нет (`content` — codec-backed `List`-поле, не child).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default `[]`.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — ОписаниеТипов (required, present всегда).
pub const F_TYPE: FieldId = FieldId(3);
/// `passwordMode` — bool. Default `false`.
pub const F_PASSWORD_MODE: FieldId = FieldId(4);
/// `format` — локализованный формат. Default `[]`.
pub const F_FORMAT: FieldId = FieldId(5);
/// `editFormat` — локализованный формат редактирования. Default `[]`.
pub const F_EDIT_FORMAT: FieldId = FieldId(6);
/// `toolTip` — локализованная подсказка. Default `[]`.
pub const F_TOOL_TIP: FieldId = FieldId(7);
/// `markNegatives` — bool. Default `false`.
pub const F_MARK_NEGATIVES: FieldId = FieldId(8);
/// `mask` — маска ввода (свободный текст). Default `""`.
pub const F_MASK: FieldId = FieldId(9);
/// `multiLine` — bool. Default `false`.
pub const F_MULTI_LINE: FieldId = FieldId(10);
/// `extendedEdit` — bool. Default `false`.
pub const F_EXTENDED_EDIT: FieldId = FieldId(11);
/// `minValue` — nullable-скаляр (required, present всегда).
pub const F_MIN_VALUE: FieldId = FieldId(12);
/// `maxValue` — nullable-скаляр (required, present всегда).
pub const F_MAX_VALUE: FieldId = FieldId(13);
/// `fillFromFillingValue` — bool. Default `false`.
pub const F_FILL_FROM_FILLING_VALUE: FieldId = FieldId(14);
/// `fillValue` — nullable-скаляр (required, present всегда).
pub const F_FILL_VALUE: FieldId = FieldId(15);
/// `fillChecking` — `{DontCheck|ShowError}`. Default `DontCheck`.
pub const F_FILL_CHECKING: FieldId = FieldId(16);
/// `choiceFoldersAndItems` — `{Items|Folders|FoldersAndItems}`. Default `Items`.
pub const F_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(17);
/// `choiceParameterLinks` — список связей параметров выбора. Default `[]`.
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(18);
/// `choiceParameters` — список параметров выбора. Default `[]`.
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(19);
/// `quickChoice` — `{Auto|Use|DontUse}`. Default `Auto`.
pub const F_QUICK_CHOICE: FieldId = FieldId(20);
/// `createOnInput` — `{Auto|Use|DontUse}`. Default `Auto`.
pub const F_CREATE_ON_INPUT: FieldId = FieldId(21);
/// `choiceForm` — ссылка на форму выбора (свободный текст). Default `""`.
pub const F_CHOICE_FORM: FieldId = FieldId(22);
/// `linkByType` — связь по типу (`Str`). Default `""`.
pub const F_LINK_BY_TYPE: FieldId = FieldId(23);
/// `choiceHistoryOnInput` — `{Auto|Use|DontUse}`. Default `Auto`.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(24);
/// `content` — состав общего реквизита: `List([List([Str(metadata), Enum(use)])])`.
/// Default `[]`.
pub const F_CONTENT: FieldId = FieldId(25);
/// `autoUse` — `{Use|DontUse}`. Default `Use`.
pub const F_AUTO_USE: FieldId = FieldId(26);
/// `dataSeparation` — `{Separate|DontUse}`. Default `Separate`.
pub const F_DATA_SEPARATION: FieldId = FieldId(27);
/// `separatedDataUse` — `{Independently|IndependentlyAndSimultaneously}`. Default
/// `Independently`.
pub const F_SEPARATED_DATA_USE: FieldId = FieldId(28);
/// `dataSeparationValue` — ссылка на параметр сеанса (свободный текст). Default `""`.
pub const F_DATA_SEPARATION_VALUE: FieldId = FieldId(29);
/// `dataSeparationUse` — ссылка на параметр сеанса (свободный текст). Default `""`.
pub const F_DATA_SEPARATION_USE: FieldId = FieldId(30);
/// `conditionalSeparation` — ссылка на объект (свободный текст). Default `""`.
pub const F_CONDITIONAL_SEPARATION: FieldId = FieldId(31);
/// `usersSeparation` — `{Separate|DontUse}`. Default `Separate`.
pub const F_USERS_SEPARATION: FieldId = FieldId(32);
/// `authenticationSeparation` — `{Separate|DontUse}`. Default `Separate`.
pub const F_AUTHENTICATION_SEPARATION: FieldId = FieldId(33);
/// `configurationExtensionsSeparation` — `{Separate|DontUse}`. Default `Separate`.
pub const F_CONFIGURATION_EXTENSIONS_SEPARATION: FieldId = FieldId(34);
/// `indexing` — `{DontIndex|Index|IndexWithAdditionalOrder}`. Default `DontIndex`.
pub const F_INDEXING: FieldId = FieldId(35);
/// `fullTextSearch` — `{DontUse|Use}`. Default `DontUse`.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(36);
/// `dataHistory` — `{DontUse|Use}`. Default `DontUse`.
pub const F_DATA_HISTORY: FieldId = FieldId(37);

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

/// Сконструировать [`FieldSpec`] вида `CommonAttribute` в каноническом (Designer DENSE)
/// порядке.
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
        e(F_CHOICE_FOLDERS_AND_ITEMS, "choiceFoldersAndItems", "Items"),
        list(F_CHOICE_PARAMETER_LINKS, "choiceParameterLinks"),
        list(F_CHOICE_PARAMETERS, "choiceParameters"),
        e(F_QUICK_CHOICE, "quickChoice", "Auto"),
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        s(F_CHOICE_FORM, "choiceForm"),
        s(F_LINK_BY_TYPE, "linkByType"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        list(F_CONTENT, "content"),
        e(F_AUTO_USE, "autoUse", "Use"),
        e(F_DATA_SEPARATION, "dataSeparation", "Separate"),
        e(F_SEPARATED_DATA_USE, "separatedDataUse", "Independently"),
        s(F_DATA_SEPARATION_VALUE, "dataSeparationValue"),
        s(F_DATA_SEPARATION_USE, "dataSeparationUse"),
        s(F_CONDITIONAL_SEPARATION, "conditionalSeparation"),
        e(F_USERS_SEPARATION, "usersSeparation", "Separate"),
        e(F_AUTHENTICATION_SEPARATION, "authenticationSeparation", "Separate"),
        e(F_CONFIGURATION_EXTENSIONS_SEPARATION, "configurationExtensionsSeparation", "Separate"),
        e(F_INDEXING, "indexing", "DontIndex"),
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonAttribute` (кэш на процесс).
pub fn common_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonAttribute",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: content — codec-backed List-поле, не child-коллекция.
    })
}
