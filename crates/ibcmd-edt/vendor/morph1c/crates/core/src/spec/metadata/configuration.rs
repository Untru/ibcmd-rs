//! Канонический спек КОРНЯ конфигурации `Configuration` (ARCHITECTURE.md §1.4/§1.6).
//! Корень — РОВНО ОДИН на конфигурацию (N=1 в SSL): vendor/version/compatibility/
//! usePurposes/defaultLanguage/множество default-form & UI-ссылок/локализованные
//! briefInformation-detailedInformation-copyright/… + `containedObjects` (платформенные
//! пары GUID) + `ChildObjects` (ИМЕНА всех объектов конфигурации — членство+порядок).
//!
//! ## Каноника N=1 (sparse EDT vs dense Designer)
//! Designer ПЛОТНЫЙ (эмитит ВСЕ ~71 свойств, в т.ч. дефолтные), EDT РАЗРЕЖЁННЫЙ (эмитит
//! лишь не-дефолтные). Канонический дефолт поля выбран так, что:
//! * поле, которое EDT ЭМИТИТ (vendor/version/synonym/…), имеет НЕЙТРАЛЬНЫЙ дефолт
//!   (пустой/false/базовый-литерал), отличный от witnessed-значения → ОБА формата
//!   эмитят значение, bag несёт его, X by construction;
//! * поле, которое EDT ОПУСКАЕТ (Designer-only: множество `Default*Form`, storages,
//!   window/interface enums), имеет дефолт = WITNESSED-значению Designer → EDT-absent
//!   восстанавливает его, bag ОБОИХ форматов его сбрасывает (== дефолт), X by
//!   construction; Designer-dense эмитит дефолт-значение byte-exact.
//! * поле, которое EDT эмитит УСЛОВНО (sparse: опускает при значении == EDT-implicit-
//!   дефолт, эмитит иначе — `synchronousPlatformExtensionAndAddInCallUseMode`,
//!   `interfaceCompatibilityMode`), имеет дефолт = WITNESSED EDT-implicit-значению (`Use`/
//!   `Taxi`, оба witnessed на min_const-пробе). НЕ нейтрал: нейтрал-`""` дал бы X-
//!   расхождение (EDT-absent `""` ↔ Designer-dense witnessed-литерал) и неверный cf-индекс.
//!
//! ## Структурно-расходящиеся (R обоих byte-exact, X-исключён `x_ignore`)
//! * `usePurposes`: EDT `PersonalComputer` (plain enum) ↔ Designer `PlatformApplication`
//!   (`<v8:Value xsi:type=…>`) — РАЗНЫЕ значения И структура; нет соответствия → x_ignore.
//! * `usedMobileApplicationFunctionalities`: EDT sparse (2) ↔ Designer dense (~50) → x_ignore.
//! * `compatibilityMode`: EDT `8.5.1` ↔ Designer `Version8_5_1` — иная кодировка → x_ignore.
//! * `languages` (EDT inline-сущность): Designer тело языка — отдельный файл → x_ignore;
//!   членство Language в ChildObjects сравнивается через `childObjects` (общий обоим).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность (каркас коннектора). `containedObjects`/
//! `ChildObjects`/`languages` — спек-СВОЙСТВА через from-root кодеки `formats-xml`.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

// --- Идентичность/локализация/EDT-эмитируемые свойства (нейтральный дефолт) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `containedObjects` — платформенные `(classId,objectId)` пары. Default [].
pub const F_CONTAINED_OBJECTS: FieldId = FieldId(2);
/// `comment` — Designer-only пустой. Default "".
pub const F_COMMENT: FieldId = FieldId(3);
/// `namePrefix` — Designer-only пустой. Default "".
pub const F_NAME_PREFIX: FieldId = FieldId(4);
/// `configurationExtensionCompatibilityMode` — Designer-only enum. Default Version8_5_1.
pub const F_EXT_COMPAT_MODE: FieldId = FieldId(5);
/// `defaultRunMode` — enum (EDT+Designer). Default "" (нейтрал → эмитят).
pub const F_DEFAULT_RUN_MODE: FieldId = FieldId(6);
/// `usePurposes` — Designer `<v8:Value>`-список. x_ignore. Default [].
pub const F_USE_PURPOSES: FieldId = FieldId(7);
/// `scriptVariant` — enum (EDT+Designer). Default "".
pub const F_SCRIPT_VARIANT: FieldId = FieldId(8);
/// `defaultRoles` — ref-list (EDT siblings ↔ Designer xr:Item). Default [].
pub const F_DEFAULT_ROLES: FieldId = FieldId(9);
/// `vendor` — string (EDT+Designer). Default "".
pub const F_VENDOR: FieldId = FieldId(10);
/// `version` — string (EDT+Designer). Default "".
pub const F_VERSION: FieldId = FieldId(11);
/// `updateCatalogAddress` — string (EDT+Designer). Default "".
pub const F_UPDATE_CATALOG_ADDRESS: FieldId = FieldId(12);
/// `includeHelpInContents` — bool (EDT+Designer). Default false.
pub const F_INCLUDE_HELP: FieldId = FieldId(13);
/// `useManagedFormInOrdinaryApplication` — Designer-only bool. Default false.
pub const F_USE_MANAGED_FORM_IN_ORDINARY: FieldId = FieldId(14);
/// `useOrdinaryFormInManagedApplication` — Designer-only bool. Default false.
pub const F_USE_ORDINARY_FORM_IN_MANAGED: FieldId = FieldId(15);
/// `additionalFullTextSearchDictionaries` — Designer-only ref/empty. Default "".
pub const F_ADDITIONAL_FTS_DICTS: FieldId = FieldId(16);
/// `commonSettingsStorage` — Designer-only ref. Default "".
pub const F_COMMON_SETTINGS_STORAGE: FieldId = FieldId(17);
/// `reportsUserSettingsStorage` — Designer-only ref. Default "".
pub const F_REPORTS_USER_SETTINGS_STORAGE: FieldId = FieldId(18);
/// `reportsVariantsStorage` — Designer-only ref. Default "".
pub const F_REPORTS_VARIANTS_STORAGE: FieldId = FieldId(19);
/// `formDataSettingsStorage` — Designer-only ref. Default "".
pub const F_FORM_DATA_SETTINGS_STORAGE: FieldId = FieldId(20);
/// `dynamicListsUserSettingsStorage` — Designer-only ref. Default "".
pub const F_DYNAMIC_LISTS_USER_SETTINGS_STORAGE: FieldId = FieldId(21);
/// `urlExternalDataStorage` — Designer-only ref. Default "".
pub const F_URL_EXTERNAL_DATA_STORAGE: FieldId = FieldId(22);
/// `content` — Designer-only ref/empty. Default "".
pub const F_CONTENT: FieldId = FieldId(23);
/// `defaultReportForm` — Designer-only ref. Default "".
pub const F_DEFAULT_REPORT_FORM: FieldId = FieldId(24);
/// `defaultReportVariantForm` — Designer-only ref. Default "".
pub const F_DEFAULT_REPORT_VARIANT_FORM: FieldId = FieldId(25);
/// `defaultReportSettingsForm` — Designer-only ref. Default "".
pub const F_DEFAULT_REPORT_SETTINGS_FORM: FieldId = FieldId(26);
/// `defaultReportAppearanceTemplate` — Designer-only ref. Default "".
pub const F_DEFAULT_REPORT_APPEARANCE_TEMPLATE: FieldId = FieldId(27);
/// `defaultDynamicListSettingsForm` — Designer-only ref. Default "".
pub const F_DEFAULT_DYNAMIC_LIST_SETTINGS_FORM: FieldId = FieldId(28);
/// `defaultSearchForm` — Designer-only ref. Default "".
pub const F_DEFAULT_SEARCH_FORM: FieldId = FieldId(29);
/// `defaultDataHistoryChangeHistoryForm` — Designer-only ref. Default "".
pub const F_DEFAULT_DH_CHANGE_HISTORY_FORM: FieldId = FieldId(30);
/// `defaultDataHistoryVersionDataForm` — Designer-only ref. Default "".
pub const F_DEFAULT_DH_VERSION_DATA_FORM: FieldId = FieldId(31);
/// `defaultDataHistoryVersionDifferencesForm` — Designer-only ref. Default "".
pub const F_DEFAULT_DH_VERSION_DIFF_FORM: FieldId = FieldId(32);
/// `defaultCollaborationSystemUsersChoiceForm` — Designer-only ref. Default "".
pub const F_DEFAULT_COLLAB_USERS_CHOICE_FORM: FieldId = FieldId(33);
/// `auxiliaryReportForm` — Designer-only ref. Default "".
pub const F_AUX_REPORT_FORM: FieldId = FieldId(34);
/// `auxiliaryReportVariantForm` — Designer-only ref. Default "".
pub const F_AUX_REPORT_VARIANT_FORM: FieldId = FieldId(35);
/// `auxiliaryReportSettingsForm` — Designer-only ref. Default "".
pub const F_AUX_REPORT_SETTINGS_FORM: FieldId = FieldId(36);
/// `auxiliaryDynamicListSettingsForm` — Designer-only ref. Default "".
pub const F_AUX_DYNAMIC_LIST_SETTINGS_FORM: FieldId = FieldId(37);
/// `auxiliaryDataHistoryChangeHistoryForm` — Designer-only ref. Default "".
pub const F_AUX_DH_CHANGE_HISTORY_FORM: FieldId = FieldId(38);
/// `auxiliaryDataHistoryVersionDataForm` — Designer-only ref. Default "".
pub const F_AUX_DH_VERSION_DATA_FORM: FieldId = FieldId(39);
/// `auxiliaryDataHistoryVersionDifferencesForm` — Designer-only ref. Default "".
pub const F_AUX_DH_VERSION_DIFF_FORM: FieldId = FieldId(40);
/// `auxiliaryCollaborationSystemUsersChoiceForm` — Designer-only ref. Default "".
pub const F_AUX_COLLAB_USERS_CHOICE_FORM: FieldId = FieldId(41);
/// `requiredMobileApplicationPermissions` — Designer-only ref/empty. Default "".
pub const F_REQUIRED_MOBILE_PERMISSIONS: FieldId = FieldId(42);
/// `usedMobileApplicationFunctionalities` — nested list, канон = EDT-sparse (Designer-кодек
/// редуцирует/разворачивает dense-таблицу). x_ignore (absent-vs-explicit-default). Default [].
pub const F_USED_MOBILE_FUNCTIONALITIES: FieldId = FieldId(43);
/// `standaloneConfigurationRestrictionRoles` — Designer-only ref/empty. Default "".
pub const F_STANDALONE_RESTRICTION_ROLES: FieldId = FieldId(44);
/// `mobileApplicationURLs` — Designer-only ref/empty. Default "".
pub const F_MOBILE_APPLICATION_URLS: FieldId = FieldId(45);
/// Ordered five-field share-request records. Default [].
pub const F_ALLOWED_INCOMING_SHARE_TYPES: FieldId = FieldId(46);
/// `mainClientApplicationWindowInterfaceVariant` — Designer-only enum. Default NavigationLeft.
pub const F_MAIN_WINDOW_INTERFACE_VARIANT: FieldId = FieldId(47);
/// `clientApplicationTheme` — Designer-only enum. Default Auto.
pub const F_CLIENT_APPLICATION_THEME: FieldId = FieldId(48);
/// `mainClientApplicationWindowMode` — Designer-only enum. Default Normal.
pub const F_MAIN_WINDOW_MODE: FieldId = FieldId(49);
/// `clientApplicationWindowsOpenVariant` — enum (EDT+Designer). Default "" (нейтрал).
pub const F_WINDOWS_OPEN_VARIANT: FieldId = FieldId(50);
/// `mainSectionPicture` — EDT-only пустой picture. x_ignore. Default "".
pub const F_MAIN_SECTION_PICTURE: FieldId = FieldId(51);
/// `defaultInterface` — Designer-only ref. Default "".
pub const F_DEFAULT_INTERFACE: FieldId = FieldId(52);
/// `caption` — Designer-only localized/empty. Default "".
pub const F_CAPTION: FieldId = FieldId(53);
/// `shortCaption` — Designer-only localized/empty. Default "".
pub const F_SHORT_CAPTION: FieldId = FieldId(54);
/// `defaultStyle` — Designer-only ref. Default "".
pub const F_DEFAULT_STYLE: FieldId = FieldId(55);
/// `defaultLanguage` — ref (EDT+Designer). Default "" (нейтрал).
pub const F_DEFAULT_LANGUAGE: FieldId = FieldId(56);
/// `briefInformation` — localized (EDT+Designer). Default [].
pub const F_BRIEF_INFORMATION: FieldId = FieldId(57);
/// `detailedInformation` — localized (EDT+Designer). Default [].
pub const F_DETAILED_INFORMATION: FieldId = FieldId(58);
/// `splash` — EDT-only пустой picture. x_ignore. Default "".
pub const F_SPLASH: FieldId = FieldId(59);
/// `copyright` — localized (EDT+Designer). Default [].
pub const F_COPYRIGHT: FieldId = FieldId(60);
/// `vendorInformationAddress` — localized (EDT+Designer). Default [].
pub const F_VENDOR_INFORMATION_ADDRESS: FieldId = FieldId(61);
/// `configurationInformationAddress` — Designer-only localized/empty. Default [].
pub const F_CONFIGURATION_INFORMATION_ADDRESS: FieldId = FieldId(62);
/// `dataLockControlMode` — enum (EDT+Designer). Default "" (нейтрал).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(63);
/// `objectAutonumerationMode` — enum (EDT+Designer). Default "" (нейтрал).
pub const F_OBJECT_AUTONUMERATION_MODE: FieldId = FieldId(64);
/// `modalityUseMode` — enum (EDT+Designer). Default "" (нейтрал).
pub const F_MODALITY_USE_MODE: FieldId = FieldId(65);
/// `synchronousPlatformExtensionAndAddInCallUseMode` — enum (EDT+Designer). Default Use
/// (WITNESSED: SSL `Use`). EDT sparse эмитит лишь != `Use` (min_const `DontUse`); SSL опускает.
pub const F_SYNC_PLATFORM_CALL_USE_MODE: FieldId = FieldId(66);
/// `interfaceCompatibilityMode` — enum (EDT+Designer). Default Taxi (WITNESSED: EDT
/// sparse-опускает при значении `Taxi` — min_const-проба; SSL `Version8_5EnableTaxi`
/// эмитят оба). НЕ нейтрал: EDT НЕ всегда эмитит это свойство → нейтрал-`""` дал бы X-
/// расхождение (EDT-absent `""` ↔ Designer `Taxi`) и неверный cf-индекс.
pub const F_INTERFACE_COMPATIBILITY_MODE: FieldId = FieldId(67);
/// `version85InterfaceMigrationMode` — Designer-only enum. Default DontUse.
pub const F_VERSION85_MIGRATION_MODE: FieldId = FieldId(68);
/// `databaseTablespacesUseMode` — Designer-only enum. Default DontUse.
pub const F_DATABASE_TABLESPACES_USE_MODE: FieldId = FieldId(69);
/// `compatibilityMode` — enum, кодировка расходится (EDT `8.5.1`/Designer `Version8_5_1`).
/// x_ignore. Default "" (нейтрал → оба эмитят свою форму).
pub const F_COMPATIBILITY_MODE: FieldId = FieldId(70);
/// `defaultConstantsForm` — Designer-only ref. Default "".
pub const F_DEFAULT_CONSTANTS_FORM: FieldId = FieldId(71);
/// `languages` — EDT-only inline-сущности языка. x_ignore. Default [].
pub const F_LANGUAGES: FieldId = FieldId(72);
/// `ChildObjects` — ИМЕНА всех объектов конфигурации (членство+порядок). Default [].
pub const F_CHILD_OBJECTS: FieldId = FieldId(73);

// --- Extension-root свойства (.cfe корень, witnessed coverage/s13_extension) ---
// ОБА формата эмитят их ТОЛЬКО на корне расширения (обычные корни s1..s12 их не несут —
// в т.ч. dense Designer!), поэтому дефолт — «отсутствует»-маркер (пустой токен / false /
// пустой список), а кодек Designer-стороны разрежён по значению (`EnumSparse`/
// `BoolPresence`), НЕ dense-эмиттер.
/// `objectBelonging` — принадлежность КОРНЯ (`Adopted` у корня расширения; обычный корень
/// узла НЕ несёт ни в одном формате). Default "" (absent-маркер). X by construction
/// (литерал `Adopted` одинаков в edt и designer).
pub const F_OBJECT_BELONGING: FieldId = FieldId(74);
/// `extension` — EDT-only структурный узел `<extension xsi:type="mdclassExtension:
/// ConfigurationExtension">` со списком Checked-флагов заимствованных свойств. Designer
/// аналога в дескрипторе не несёт → x_ignore. IR: List([Str(имя-флага)…]). Default [].
pub const F_EXTENSION: FieldId = FieldId(75);
/// `keepMappingToExtendedConfigurationObjectsByIDs` — bool корня расширения (оба формата:
/// текст `true` при истине, отсутствует при false — presence-кодировка). Default false.
pub const F_KEEP_MAPPING: FieldId = FieldId(76);
/// `configurationExtensionPurpose` — назначение расширения (`Customization`; только корень
/// расширения). Default "" (absent-маркер). Литерал одинаков в edt и designer.
pub const F_EXT_PURPOSE: FieldId = FieldId(77);
/// Typed source help-presence marker, paired with actual help-page sidecars.
pub const F_HELP: FieldId = FieldId(78);
/// Root MdPicture Logo, two nullable persisted Points plus attached image.
pub const F_LOGO: FieldId = FieldId(79);

/// Witnessed Checked-флаги `<extension>` корня расширения (s13_extension/Configuration.mdo,
/// назначение Customization): заимствованные свойства корня, которые EDT помечает
/// `Checked`. Designer-дескриптор аналога НЕ несёт (поле x_ignore), поэтому
/// designer→edt СИНТЕЗИРУЕТ ровно этот witnessed-набор (см. edt-коннектор Configuration).
pub const EXTENSION_WITNESSED_FLAGS: &[&str] = &[
    "defaultRunMode",
    "usePurposes",
    "defaultLanguage",
    "modalityUseMode",
    "interfaceCompatibilityMode",
    "compatibilityMode",
];

/// Shape-дискриминатор КОРНЯ конфигурации: расширение (.cfe) ⟺ bag несёт
/// `objectBelonging == Adopted` (оба формата) ЛИБО непустой `<extension>`-блок
/// (EDT-only). Обычный корень (.cf) не несёт ни того, ни другого (absent-маркер-дефолты
/// сжимаются из bag на чтении) — дискриминация детерминированна по IR, без эвристик.
pub fn is_extension_root(bag: &[(FieldId, PropertyValue)]) -> bool {
    bag.iter().any(|(id, v)| match (*id, v) {
        (F_OBJECT_BELONGING, PropertyValue::Enum(tok)) => tok.as_str() == "Adopted",
        (F_EXTENSION, PropertyValue::List(flags)) => !flags.is_empty(),
        _ => false,
    })
}

fn empty_str() -> PropertyValue {
    PropertyValue::Str(String::new())
}
/// An empty outer List denotes absence; present MdPicture has transparentPixel/glyph nullable Point lists.
fn picture_absent() -> PropertyValue {
    PropertyValue::List(Vec::new())
}
fn empty_loc() -> PropertyValue {
    PropertyValue::Localized(Vec::new())
}
fn empty_list() -> PropertyValue {
    PropertyValue::List(Vec::new())
}
fn enum_tok(s: &str) -> PropertyValue {
    PropertyValue::Enum(Token::new(s))
}
fn bool_f() -> PropertyValue {
    PropertyValue::Bool(false)
}

fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, empty_loc())
        .normalized(Normalize::LocalizedSortByLang)
}
fn str_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, empty_str())
}
/// Designer-only ref/empty: дефолт "" (EDT опускает; Designer dense эмитит `<Tag/>`).
fn dref(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, empty_str())
}
/// Designer-only enum с witnessed-значением как дефолтом (EDT опускает, X by construction).
fn denum(id: FieldId, name: &'static str, lit: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, enum_tok(lit))
}
/// Designer-only bool с witnessed false-дефолтом.
fn dbool(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, bool_f())
}
/// EDT+Designer enum: НЕЙТРАЛЬНЫЙ дефолт "" → оба эмитят witnessed-значение.
fn neutral_enum(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, enum_tok(""))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        // objectBelonging — ПЕРВОЕ свойство Designer-Properties корня расширения
        // (до Name/Synonym); обычные корни его не несут (absent-маркер "" — дефолт).
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, enum_tok("")),
        // extension — EDT-only Checked-флаги заимствованных свойств корня расширения.
        FieldSpec::with_default(F_EXTENSION, "extension", ValueKind::List, empty_list()).x_ignored(),
        loc(F_SYNONYM, "synonym"),
        FieldSpec::with_default(F_CONTAINED_OBJECTS, "containedObjects", ValueKind::List, empty_list()),
        str_f(F_COMMENT, "comment"),
        // extension-root пара (Designer-порядок: … Comment → ConfigurationExtensionPurpose →
        // KeepMapping… → NamePrefix; обычные корни обоих форматов их НЕ несут).
        FieldSpec::with_default(F_EXT_PURPOSE, "configurationExtensionPurpose", ValueKind::Enum, enum_tok("")),
        FieldSpec::with_default(F_KEEP_MAPPING, "keepMappingToExtendedConfigurationObjectsByIDs", ValueKind::Bool, bool_f()),
        str_f(F_NAME_PREFIX, "namePrefix"),
        // configurationExtensionCompatibilityMode: кодировка расходится (EDT `8.5.1` /
        // Designer `Version8_5_1`) → канон = EDT-dotted, Designer транслирует
        // (`Codec::CompatibilityMode`, как compatibilityMode). Дефолт = witnessed `8.5.1`
        // (Designer dense эмитит его всегда; EDT-absent восстанавливает).
        denum(F_EXT_COMPAT_MODE, "configurationExtensionCompatibilityMode", "8.5.1"),
        neutral_enum(F_DEFAULT_RUN_MODE, "defaultRunMode"),
        // usePurposes: структурно расходится EDT/Designer → x_ignore. Default [].
        FieldSpec::with_default(F_USE_PURPOSES, "usePurposes", ValueKind::List, empty_list()).x_ignored(),
        neutral_enum(F_SCRIPT_VARIANT, "scriptVariant"),
        FieldSpec::with_default(F_DEFAULT_ROLES, "defaultRoles", ValueKind::List, empty_list()),
        str_f(F_VENDOR, "vendor"),
        str_f(F_VERSION, "version"),
        str_f(F_UPDATE_CATALOG_ADDRESS, "updateCatalogAddress"),
        FieldSpec::with_default(F_INCLUDE_HELP, "includeHelpInContents", ValueKind::Bool, bool_f()),
        dbool(F_USE_MANAGED_FORM_IN_ORDINARY, "useManagedFormInOrdinaryApplication"),
        dbool(F_USE_ORDINARY_FORM_IN_MANAGED, "useOrdinaryFormInManagedApplication"),
        dref(F_ADDITIONAL_FTS_DICTS, "additionalFullTextSearchDictionaries"),
        dref(F_COMMON_SETTINGS_STORAGE, "commonSettingsStorage"),
        dref(F_REPORTS_USER_SETTINGS_STORAGE, "reportsUserSettingsStorage"),
        dref(F_REPORTS_VARIANTS_STORAGE, "reportsVariantsStorage"),
        dref(F_FORM_DATA_SETTINGS_STORAGE, "formDataSettingsStorage"),
        dref(F_DYNAMIC_LISTS_USER_SETTINGS_STORAGE, "dynamicListsUserSettingsStorage"),
        dref(F_URL_EXTERNAL_DATA_STORAGE, "urlExternalDataStorage"),
        dref(F_CONTENT, "content"),
        dref(F_DEFAULT_REPORT_FORM, "defaultReportForm"),
        dref(F_DEFAULT_REPORT_VARIANT_FORM, "defaultReportVariantForm"),
        dref(F_DEFAULT_REPORT_SETTINGS_FORM, "defaultReportSettingsForm"),
        dref(F_DEFAULT_REPORT_APPEARANCE_TEMPLATE, "defaultReportAppearanceTemplate"),
        dref(F_DEFAULT_DYNAMIC_LIST_SETTINGS_FORM, "defaultDynamicListSettingsForm"),
        dref(F_DEFAULT_SEARCH_FORM, "defaultSearchForm"),
        dref(F_DEFAULT_DH_CHANGE_HISTORY_FORM, "defaultDataHistoryChangeHistoryForm"),
        dref(F_DEFAULT_DH_VERSION_DATA_FORM, "defaultDataHistoryVersionDataForm"),
        dref(F_DEFAULT_DH_VERSION_DIFF_FORM, "defaultDataHistoryVersionDifferencesForm"),
        dref(F_DEFAULT_COLLAB_USERS_CHOICE_FORM, "defaultCollaborationSystemUsersChoiceForm"),
        dref(F_AUX_REPORT_FORM, "auxiliaryReportForm"),
        dref(F_AUX_REPORT_VARIANT_FORM, "auxiliaryReportVariantForm"),
        dref(F_AUX_REPORT_SETTINGS_FORM, "auxiliaryReportSettingsForm"),
        dref(F_AUX_DYNAMIC_LIST_SETTINGS_FORM, "auxiliaryDynamicListSettingsForm"),
        dref(F_AUX_DH_CHANGE_HISTORY_FORM, "auxiliaryDataHistoryChangeHistoryForm"),
        dref(F_AUX_DH_VERSION_DATA_FORM, "auxiliaryDataHistoryVersionDataForm"),
        dref(F_AUX_DH_VERSION_DIFF_FORM, "auxiliaryDataHistoryVersionDifferencesForm"),
        dref(F_AUX_COLLAB_USERS_CHOICE_FORM, "auxiliaryCollaborationSystemUsersChoiceForm"),
        dref(F_REQUIRED_MOBILE_PERMISSIONS, "requiredMobileApplicationPermissions"),
        // usedMobileApplicationFunctionalities: канон = EDT-sparse (только use=true строки,
        // ""=Biometrics); Designer-кодек редуцирует dense→sparse на read (§1.0-самопроверка
        // re-expand) и разворачивает sparse→dense на write → кросс-формат ПЕРЕНОСИТ значение
        // byte-exact (s15 e2d/d2e 0-diff). x_ignore ОСТАЁТСЯ: платформенный дефолт значения —
        // НЕ пустой ({Biometrics,OSBackup}=true), поэтому рукописный Designer-src БЕЗ элемента
        // (probes/min_const) и платформенный EDT-дамп С явным дефолт-блоком несут одно значение
        // в разных «absent vs explicit» кодировках — X по бэгу свойств здесь неразрешим без
        // модели платформенных дефолтов.
        FieldSpec::with_default(
            F_USED_MOBILE_FUNCTIONALITIES,
            "usedMobileApplicationFunctionalities",
            ValueKind::List,
            empty_list(),
        )
        .x_ignored(),
        dref(F_STANDALONE_RESTRICTION_ROLES, "standaloneConfigurationRestrictionRoles"),
        dref(F_MOBILE_APPLICATION_URLS, "mobileApplicationURLs"),
        FieldSpec::with_default(F_ALLOWED_INCOMING_SHARE_TYPES, "allowedIncomingShareRequestTypes", ValueKind::List, empty_list()),
        denum(F_MAIN_WINDOW_MODE, "mainClientApplicationWindowMode", "Normal"),
        denum(F_CLIENT_APPLICATION_THEME, "clientApplicationTheme", "Auto"),
        denum(F_MAIN_WINDOW_INTERFACE_VARIANT, "mainClientApplicationWindowInterfaceVariant", "NavigationLeft"),
        neutral_enum(F_WINDOWS_OPEN_VARIANT, "clientApplicationWindowsOpenVariant"),
        // mainSectionPicture: EDT-only пустой узел `<mainSectionPicture/>`, present РОВНО
        // при наличии картинки (см. `picture_absent`); Designer не несёт → x_ignore.
        FieldSpec::with_default(F_MAIN_SECTION_PICTURE, "mainSectionPicture", ValueKind::List, picture_absent())
            .x_ignored(),
        dref(F_DEFAULT_INTERFACE, "defaultInterface"),
        loc(F_CAPTION, "caption"),
        loc(F_SHORT_CAPTION, "shortCaption"),
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        dref(F_DEFAULT_STYLE, "defaultStyle"),
        str_f(F_DEFAULT_LANGUAGE, "defaultLanguage"),
        loc(F_BRIEF_INFORMATION, "briefInformation"),
        loc(F_DETAILED_INFORMATION, "detailedInformation"),
        // splash: EDT-only пустой узел `<splash/>`, present РОВНО при наличии картинки
        // (см. `picture_absent`); Designer не несёт → x_ignore.
        FieldSpec::with_default(F_SPLASH, "splash", ValueKind::List, picture_absent()).x_ignored(),
        FieldSpec::with_default(F_LOGO, "logo", ValueKind::List, picture_absent()).x_ignored(),
        loc(F_COPYRIGHT, "copyright"),
        loc(F_VENDOR_INFORMATION_ADDRESS, "vendorInformationAddress"),
        loc(F_CONFIGURATION_INFORMATION_ADDRESS, "configurationInformationAddress"),
        neutral_enum(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode"),
        neutral_enum(F_OBJECT_AUTONUMERATION_MODE, "objectAutonumerationMode"),
        neutral_enum(F_MODALITY_USE_MODE, "modalityUseMode"),
        denum(F_SYNC_PLATFORM_CALL_USE_MODE, "synchronousPlatformExtensionAndAddInCallUseMode", "Use"),
        // interfaceCompatibilityMode: EDT sparse-опускает при `Taxi` (min_const) → канон-
        // дефолт = witnessed `Taxi` (denum), НЕ нейтрал. Иначе EDT-absent дал бы `""` → X-
        // расхождение с Designer `Taxi` (§1.6) и cf-индекс по умолчанию ушёл бы в SSL-6.
        denum(F_INTERFACE_COMPATIBILITY_MODE, "interfaceCompatibilityMode", "Taxi"),
        denum(F_VERSION85_MIGRATION_MODE, "version85InterfaceMigrationMode", "DontUse"),
        denum(F_DATABASE_TABLESPACES_USE_MODE, "databaseTablespacesUseMode", "DontUse"),
        // compatibilityMode: иная кодировка EDT/Designer → x_ignore (нейтрал дефолт → оба эмитят).
        neutral_enum(F_COMPATIBILITY_MODE, "compatibilityMode").x_ignored(),
        dref(F_DEFAULT_CONSTANTS_FORM, "defaultConstantsForm"),
        // languages: EDT-only inline-сущности; Designer держит тело языка отдельным файлом → x_ignore.
        FieldSpec::with_default(F_LANGUAGES, "languages", ValueKind::List, empty_list()).x_ignored(),
        FieldSpec::with_default(F_CHILD_OBJECTS, "ChildObjects", ValueKind::List, empty_list()),
    ]
}

/// Канонический [`EntitySpec`] корня `Configuration` (кэш на процесс).
pub fn configuration() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Configuration",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // childObjects — НЕ рекурсия (ИМЕНА-ссылки), а спек-свойство `ChildObjects`.
    })
}
