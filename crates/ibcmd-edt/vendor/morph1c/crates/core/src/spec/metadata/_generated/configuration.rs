//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Configuration`, guid=cf4abeab-37b2-11d4-940f-008048da11f9.
//!
//! НЕ КОММИТИТЬ КАК ЕСТЬ (§1.0): порядок полей = EMF/.mdo (supertypes-first); Designer
//! DENSE-порядок и дефолты ВЕРИФИЦИРОВАТЬ R/X-гейтом + дифф-фикстурами (docs/RESEARCH.md).
//! `name`/`uuid` — идентичность, вне спека. Все `// TODO(draft)` — проверить.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};
use crate::version::{FormatVersion, Since};

/// `synonym`  guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment`  guid=cf4abea4-37b2-11d4-940f-008048da11f9
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging`  guid=19744814-daec-423b-8269-995b53ebe0ec
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject`  guid=9595ddd6-e72c-47ad-a156-672db811628c
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `keepMappingToExtendedConfigurationObjectsByIDs` (since 8.3.15)  guid=47f509d7-3ba3-46c8-8fd8-fbeb3662c0d9
pub const F_KEEP_MAPPING_TO_EXTENDED_CONFIGURATION_OBJECTS_BY_I_DS: FieldId = FieldId(5);
/// `namePrefix`  guid=52547ac3-1dff-4930-88b8-7419997945ba
pub const F_NAME_PREFIX: FieldId = FieldId(6);
/// `configurationExtensionCompatibilityMode`  guid=c79cb2e3-6fa1-446a-97f6-68b4742480b5
pub const F_CONFIGURATION_EXTENSION_COMPATIBILITY_MODE: FieldId = FieldId(7);
/// `configurationExtensionPurpose` (since 8.3.10)  guid=67363886-e95e-403b-9127-82cf64183fab
pub const F_CONFIGURATION_EXTENSION_PURPOSE: FieldId = FieldId(8);
/// `defaultRunMode`  guid=c6c6cdec-9de1-431f-b17a-e442eebf86c1
pub const F_DEFAULT_RUN_MODE: FieldId = FieldId(9);
/// `usePurposes`  guid=da648ef9-2f12-418f-8e2f-8956bc10a66f
pub const F_USE_PURPOSES: FieldId = FieldId(10);
/// `scriptVariant`  guid=2b144fd9-4ab7-4f9e-a5a9-753d43a9840e
pub const F_SCRIPT_VARIANT: FieldId = FieldId(11);
/// `defaultRole`  guid=f6cdb7fb-dab4-4699-a22e-feac3e4a2cae
pub const F_DEFAULT_ROLE: FieldId = FieldId(12);
/// `defaultRoles`  guid=6a447e3f-d9d7-4c97-a239-87e3fe6d8055
pub const F_DEFAULT_ROLES: FieldId = FieldId(13);
/// `vendor`  guid=c5f8bc62-5534-499e-ac1a-d7d043df2a9e
pub const F_VENDOR: FieldId = FieldId(14);
/// `version`  guid=b8c86b25-2e11-4be1-9cd3-9a9cd6ae0f7a
pub const F_VERSION: FieldId = FieldId(15);
/// `updateCatalogAddress`  guid=b95a55fd-5334-45a9-8189-fdcbb6d71ade
pub const F_UPDATE_CATALOG_ADDRESS: FieldId = FieldId(16);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(17);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(18);
/// `useManagedFormInOrdinaryApplication`  guid=da451f2d-2b84-4c22-a1b4-f3ff02bc87a7
pub const F_USE_MANAGED_FORM_IN_ORDINARY_APPLICATION: FieldId = FieldId(19);
/// `useOrdinaryFormInManagedApplication`  guid=fe4e2ec0-716b-42ab-a1c4-dfa8dd242b3d
pub const F_USE_ORDINARY_FORM_IN_MANAGED_APPLICATION: FieldId = FieldId(20);
/// `additionalFullTextSearchDictionaries`  guid=24ff84da-3683-4c92-8bcd-03f6eb5a208b
pub const F_ADDITIONAL_FULL_TEXT_SEARCH_DICTIONARIES: FieldId = FieldId(21);
/// `commonSettingsStorage`  guid=982ff47a-aba8-41b2-aedb-cde5e10291a2
pub const F_COMMON_SETTINGS_STORAGE: FieldId = FieldId(22);
/// `reportsUserSettingsStorage`  guid=932a87cf-144c-449f-970f-19835d547068
pub const F_REPORTS_USER_SETTINGS_STORAGE: FieldId = FieldId(23);
/// `reportsVariantsStorage`  guid=ce176ad6-af25-4ec7-a660-750772e493ff
pub const F_REPORTS_VARIANTS_STORAGE: FieldId = FieldId(24);
/// `formDataSettingsStorage`  guid=7afdfce2-d83a-4762-b863-06f720de2d86
pub const F_FORM_DATA_SETTINGS_STORAGE: FieldId = FieldId(25);
/// `dynamicListsUserSettingsStorage`  guid=ce445614-63e1-43ba-910d-72105140e94f
pub const F_DYNAMIC_LISTS_USER_SETTINGS_STORAGE: FieldId = FieldId(26);
/// `urlExternalDataStorage` (since 8.3.19)  guid=43a0ac9c-210e-4571-af7e-6ebab49a4dce
pub const F_URL_EXTERNAL_DATA_STORAGE: FieldId = FieldId(27);
/// `content`  guid=c6690627-40cf-4741-8719-4e7feb832b84
pub const F_CONTENT: FieldId = FieldId(28);
/// `defaultReportForm`  guid=3858755b-07b8-4c0b-a7b1-b013192a5863
pub const F_DEFAULT_REPORT_FORM: FieldId = FieldId(29);
/// `defaultReportVariantForm`  guid=700a5dda-3341-45c2-974e-3c8bfb318228
pub const F_DEFAULT_REPORT_VARIANT_FORM: FieldId = FieldId(30);
/// `defaultReportSettingsForm`  guid=a78774fa-ae3b-4450-a0d1-22d5654af1bb
pub const F_DEFAULT_REPORT_SETTINGS_FORM: FieldId = FieldId(31);
/// `defaultDynamicListSettingsForm`  guid=f71e5124-a281-4192-bc51-3be41a230a11
pub const F_DEFAULT_DYNAMIC_LIST_SETTINGS_FORM: FieldId = FieldId(32);
/// `defaultSearchForm`  guid=5e3c8a23-7453-4785-a338-2c0d78700e74
pub const F_DEFAULT_SEARCH_FORM: FieldId = FieldId(33);
/// `defaultDataHistoryChangeHistoryForm` (since 8.3.13)  guid=770647c-f103-4c3a-a834-144df18a2938
pub const F_DEFAULT_DATA_HISTORY_CHANGE_HISTORY_FORM: FieldId = FieldId(34);
/// `defaultDataHistoryVersionDataForm` (since 8.3.13)  guid=5e815a7a-f20f-4a57-a5d0-01c2ceba0dbe
pub const F_DEFAULT_DATA_HISTORY_VERSION_DATA_FORM: FieldId = FieldId(35);
/// `defaultDataHistoryVersionDifferencesForm` (since 8.3.13)  guid=6c487f40-583b-4100-ac4b-7f9ab2678557
pub const F_DEFAULT_DATA_HISTORY_VERSION_DIFFERENCES_FORM: FieldId = FieldId(36);
/// `defaultCollaborationSystemUsersChoiceForm` (since 8.3.15)  guid=2e69c9c8-7383-438f-a424-29ebaa7dc015
pub const F_DEFAULT_COLLABORATION_SYSTEM_USERS_CHOICE_FORM: FieldId = FieldId(37);
/// `auxiliaryReportForm` (since 8.5.1)  guid=e0fc09aa-cccb-45ec-b0b2-9467af5d05de
pub const F_AUXILIARY_REPORT_FORM: FieldId = FieldId(38);
/// `auxiliaryReportVariantForm` (since 8.5.1)  guid=ebff9bec-13bb-49de-934b-e9e0a0f859da
pub const F_AUXILIARY_REPORT_VARIANT_FORM: FieldId = FieldId(39);
/// `auxiliaryReportSettingsForm` (since 8.5.1)  guid=1a0ae3f2-3f27-410b-acba-d9ee82e32c10
pub const F_AUXILIARY_REPORT_SETTINGS_FORM: FieldId = FieldId(40);
/// `auxiliaryDynamicListSettingsForm` (since 8.5.1)  guid=93303a02-5bd8-4864-bf74-abda2fd24d62
pub const F_AUXILIARY_DYNAMIC_LIST_SETTINGS_FORM: FieldId = FieldId(41);
/// `auxiliaryDataHistoryChangeHistoryForm` (since 8.5.1)  guid=7e674d01-addd-41dc-a2a8-527ef98a8dcb
pub const F_AUXILIARY_DATA_HISTORY_CHANGE_HISTORY_FORM: FieldId = FieldId(42);
/// `auxiliaryDataHistoryVersionDataForm` (since 8.5.1)  guid=f68f24d0-dfa6-4ee9-b4d0-de64ddabf0e4
pub const F_AUXILIARY_DATA_HISTORY_VERSION_DATA_FORM: FieldId = FieldId(43);
/// `auxiliaryDataHistoryVersionDifferencesForm` (since 8.5.1)  guid=f52f3599-b4f1-47be-831c-f10be5b069b2
pub const F_AUXILIARY_DATA_HISTORY_VERSION_DIFFERENCES_FORM: FieldId = FieldId(44);
/// `auxiliaryCollaborationSystemUsersChoiceForm` (since 8.5.1)  guid=3397beb5-1107-4def-9183-c867cab2d4c0
pub const F_AUXILIARY_COLLABORATION_SYSTEM_USERS_CHOICE_FORM: FieldId = FieldId(45);
/// `requiredMobileApplicationPermissions`  guid=3431bd92-4a73-4ace-aace-191f9fdeaea2
pub const F_REQUIRED_MOBILE_APPLICATION_PERMISSIONS: FieldId = FieldId(46);
/// `usedMobileApplicationFunctionalities`  guid=cca11de4-1e73-4988-8c35-2ae49b6c616c
pub const F_USED_MOBILE_APPLICATION_FUNCTIONALITIES: FieldId = FieldId(47);
/// `standaloneConfigurationRestrictionRoles` (since 8.3.16)  guid=39bfa4e0-334f-428c-803b-9227445f1788
pub const F_STANDALONE_CONFIGURATION_RESTRICTION_ROLES: FieldId = FieldId(48);
/// `*/`  guid=740eb5f6-e214-4e30-aefa-f15bab91c688
pub const F_*/: FieldId = FieldId(49);
/// `>` (since 8.3.9)  guid=740eb5f6-e214-4e30-aefa-f15bab91c688
pub const F_>: FieldId = FieldId(50);
/// `clientApplicationTheme` (since 8.5.1)  guid=ac932464-aba3-4ec5-5d5e-ea639daa8649
pub const F_CLIENT_APPLICATION_THEME: FieldId = FieldId(51);
/// `mainClientApplicationWindowInterfaceVariant` (since 8.5.1)  guid=2fa2e50c-2840-4073-8b50-e6b78d8a5112
pub const F_MAIN_CLIENT_APPLICATION_WINDOW_INTERFACE_VARIANT: FieldId = FieldId(52);
/// `clientApplicationWindowsOpenVariant` (since 8.5.1)  guid=bf88448f-704c-4fcd-ad70-9321aa09723a
pub const F_CLIENT_APPLICATION_WINDOWS_OPEN_VARIANT: FieldId = FieldId(53);
/// `mainSectionPicture`  guid=6dece861-bb9b-4acd-a35e-5b514531abe1
pub const F_MAIN_SECTION_PICTURE: FieldId = FieldId(54);
/// `caption` (since 8.5.1)  guid=4d8ee908-bd04-4026-8060-55223e2e7743
pub const F_CAPTION: FieldId = FieldId(55);
/// `shortCaption` (since 8.5.1)  guid=f514ae41-2bf3-4c6b-b1d1-ca69450facbc
pub const F_SHORT_CAPTION: FieldId = FieldId(56);
/// `defaultStyle`  guid=ac8f7099-e6c4-4a11-aa97-26105d03cfbc
pub const F_DEFAULT_STYLE: FieldId = FieldId(57);
/// `defaultLanguage`  guid=15e3462b-bc9b-40cd-9d9f-dc0922f84560
pub const F_DEFAULT_LANGUAGE: FieldId = FieldId(58);
/// `briefInformation`  guid=12fce844-eac7-4413-8235-19256a766f21
pub const F_BRIEF_INFORMATION: FieldId = FieldId(59);
/// `detailedInformation`  guid=a91a4a6e-0268-4ee2-b48c-9986c8c1dbd9
pub const F_DETAILED_INFORMATION: FieldId = FieldId(60);
/// `logo`  guid=3035a9db-d6b2-450e-8b22-4430577f8dab
pub const F_LOGO: FieldId = FieldId(61);
/// `splash`  guid=d98a8e01-7219-41ce-9f23-dada0860b9bf
pub const F_SPLASH: FieldId = FieldId(62);
/// `copyright`  guid=876bcf1c-e28d-4749-8619-6138739eb433
pub const F_COPYRIGHT: FieldId = FieldId(63);
/// `vendorInformationAddress`  guid=cc298508-6a00-422b-a1a0-e2fb933fb408
pub const F_VENDOR_INFORMATION_ADDRESS: FieldId = FieldId(64);
/// `configurationInformationAddress`  guid=2efeb7da-4876-464e-9b46-fe7fd5554a8e
pub const F_CONFIGURATION_INFORMATION_ADDRESS: FieldId = FieldId(65);
/// `mainClientApplicationWindowMode` (since 8.3.10)  guid=648d83d8-0468-49a5-a50f-070e7846a231
pub const F_MAIN_CLIENT_APPLICATION_WINDOW_MODE: FieldId = FieldId(66);
/// `dataLockControlMode`  guid=eb610a9d-686b-4e90-8629-8713c914aa2d
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(67);
/// `binaryDataStorageMode` (since 8.3.23)  guid=e20689a6-b18e-43af-8490-49476b8f216e
pub const F_BINARY_DATA_STORAGE_MODE: FieldId = FieldId(68);
/// `binaryDataBlockStorageUseMode` (since 8.3.26)  guid=26af5caf-772e-478e-9fe5-156642df6d53
pub const F_BINARY_DATA_BLOCK_STORAGE_USE_MODE: FieldId = FieldId(69);
/// `objectAutonumerationMode`  guid=74fc3305-e7f0-4389-918e-f74a105d316c
pub const F_OBJECT_AUTONUMERATION_MODE: FieldId = FieldId(70);
/// `modalityUseMode`  guid=3a5cca5c-0675-43df-80ca-2cf5163335f2
pub const F_MODALITY_USE_MODE: FieldId = FieldId(71);
/// `interfaceCompatibilityMode`  guid=cbd1f1ed-72d3-4da3-bd02-c643d9595b7d
pub const F_INTERFACE_COMPATIBILITY_MODE: FieldId = FieldId(72);
/// `version85InterfaceMigrationMode` (since 8.5.1)  guid=5c46b26c-d9b5-4d77-bf05-cdbb7178af13
pub const F_VERSION85_INTERFACE_MIGRATION_MODE: FieldId = FieldId(73);
/// `databaseTablespacesUseMode` (since 8.3.23)  guid=9497ea6f-b3d2-433d-ab38-7c562e2932ab
pub const F_DATABASE_TABLESPACES_USE_MODE: FieldId = FieldId(74);
/// `compatibilityMode`  guid=161bc4c0-4cc0-4382-a045-f58f25e24783
pub const F_COMPATIBILITY_MODE: FieldId = FieldId(75);
/// `defaultConstantsForm`  guid=d4769b90-a0ff-11d5-b99f-0050bae0a95d
pub const F_DEFAULT_CONSTANTS_FORM: FieldId = FieldId(76);
/// `defaultReportAppearanceTemplate`  guid=b73fb5ab-f9e5-4855-8b20-8c1b12f0d41b
pub const F_DEFAULT_REPORT_APPEARANCE_TEMPLATE: FieldId = FieldId(77);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
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
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        b(F_KEEP_MAPPING_TO_EXTENDED_CONFIGURATION_OBJECTS_BY_I_DS, "keepMappingToExtendedConfigurationObjectsByIDs"),
        s(F_NAME_PREFIX, "namePrefix"),
        e(F_CONFIGURATION_EXTENSION_COMPATIBILITY_MODE, "configurationExtensionCompatibilityMode", ""),  // literals: {}
        e(F_CONFIGURATION_EXTENSION_PURPOSE, "configurationExtensionPurpose", "Patch"),  // literals: {Patch|Customization|AddOn}
        e(F_DEFAULT_RUN_MODE, "defaultRunMode", "Auto"),  // literals: {Auto|ManagedApplication|OrdinaryApplication}
        e(F_USE_PURPOSES, "usePurposes", "PersonalComputer"),  // literals: {PersonalComputer|MobileDevice}
        e(F_SCRIPT_VARIANT, "scriptVariant", "English"),  // literals: {English|Russian}
        s(F_DEFAULT_ROLE, "defaultRole"),  // TODO(draft): ref -> Role (form-ref Str vs Ref?)
        list(F_DEFAULT_ROLES, "defaultRoles"),  // TODO(draft): ref-list -> Role
        s(F_VENDOR, "vendor"),
        s(F_VERSION, "version"),
        s(F_UPDATE_CATALOG_ADDRESS, "updateCatalogAddress"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        b(F_USE_MANAGED_FORM_IN_ORDINARY_APPLICATION, "useManagedFormInOrdinaryApplication"),
        b(F_USE_ORDINARY_FORM_IN_MANAGED_APPLICATION, "useOrdinaryFormInManagedApplication"),
        list(F_ADDITIONAL_FULL_TEXT_SEARCH_DICTIONARIES, "additionalFullTextSearchDictionaries"),  // TODO(draft): ref-list -> FullTextSearchDictionarySource
        s(F_COMMON_SETTINGS_STORAGE, "commonSettingsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        s(F_REPORTS_USER_SETTINGS_STORAGE, "reportsUserSettingsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        s(F_REPORTS_VARIANTS_STORAGE, "reportsVariantsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        s(F_FORM_DATA_SETTINGS_STORAGE, "formDataSettingsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        s(F_DYNAMIC_LISTS_USER_SETTINGS_STORAGE, "dynamicListsUserSettingsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        s(F_URL_EXTERNAL_DATA_STORAGE, "urlExternalDataStorage")
            .gated(Since(FormatVersion::new(2, 12))),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        list(F_CONTENT, "content"),  // TODO(draft): ref-list -> MdObject
        s(F_DEFAULT_REPORT_FORM, "defaultReportForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_REPORT_VARIANT_FORM, "defaultReportVariantForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_REPORT_SETTINGS_FORM, "defaultReportSettingsForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_DYNAMIC_LIST_SETTINGS_FORM, "defaultDynamicListSettingsForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_SEARCH_FORM, "defaultSearchForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_DATA_HISTORY_CHANGE_HISTORY_FORM, "defaultDataHistoryChangeHistoryForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_DATA_HISTORY_VERSION_DATA_FORM, "defaultDataHistoryVersionDataForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_DATA_HISTORY_VERSION_DIFFERENCES_FORM, "defaultDataHistoryVersionDifferencesForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_COLLABORATION_SYSTEM_USERS_CHOICE_FORM, "defaultCollaborationSystemUsersChoiceForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_REPORT_FORM, "auxiliaryReportForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_REPORT_VARIANT_FORM, "auxiliaryReportVariantForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_REPORT_SETTINGS_FORM, "auxiliaryReportSettingsForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_DYNAMIC_LIST_SETTINGS_FORM, "auxiliaryDynamicListSettingsForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_DATA_HISTORY_CHANGE_HISTORY_FORM, "auxiliaryDataHistoryChangeHistoryForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_DATA_HISTORY_VERSION_DATA_FORM, "auxiliaryDataHistoryVersionDataForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_DATA_HISTORY_VERSION_DIFFERENCES_FORM, "auxiliaryDataHistoryVersionDifferencesForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_COLLABORATION_SYSTEM_USERS_CHOICE_FORM, "auxiliaryCollaborationSystemUsersChoiceForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        e(F_REQUIRED_MOBILE_APPLICATION_PERMISSIONS, "requiredMobileApplicationPermissions", "Multimedia"),  // literals: {Multimedia|Location|Contacts|Calendars|Telephony|PushNotification|LocalNotification|Print|InAppPurchases}
        s(F_USED_MOBILE_APPLICATION_FUNCTIONALITIES, "usedMobileApplicationFunctionalities"),  // TODO(draft): contained UsedFunctionality — уточнить vk
        list(F_STANDALONE_CONFIGURATION_RESTRICTION_ROLES, "standaloneConfigurationRestrictionRoles"),  // TODO(draft): ref-list -> Role
        s(F_*/, "*/"),  // TODO(draft): contained /* Use this if version — уточнить vk
        s(F_>, ">"),  // TODO(draft): contained /* Use this if version — уточнить vk
        e(F_CLIENT_APPLICATION_THEME, "clientApplicationTheme", "Auto")
            .gated(Since(FormatVersion::new(2, 21))),  // literals: {Auto|Light|Dark}
        e(F_MAIN_CLIENT_APPLICATION_WINDOW_INTERFACE_VARIANT, "mainClientApplicationWindowInterfaceVariant", "NavigationLeft")
            .gated(Since(FormatVersion::new(2, 21))),  // literals: {NavigationLeft|NavigationTop}
        e(F_CLIENT_APPLICATION_WINDOWS_OPEN_VARIANT, "clientApplicationWindowsOpenVariant", "OpenDataInTabs")
            .gated(Since(FormatVersion::new(2, 21))),  // literals: {OpenDataInTabs|OpenDataInDialogs}
        s(F_MAIN_SECTION_PICTURE, "mainSectionPicture"),  // TODO(draft): contained MdPicture — уточнить vk
        loc_field(F_CAPTION, "caption")
            .gated(Since(FormatVersion::new(2, 21))),
        loc_field(F_SHORT_CAPTION, "shortCaption")
            .gated(Since(FormatVersion::new(2, 21))),
        s(F_DEFAULT_STYLE, "defaultStyle"),  // TODO(draft): ref -> Style (form-ref Str vs Ref?)
        s(F_DEFAULT_LANGUAGE, "defaultLanguage"),  // TODO(draft): ref -> Language (form-ref Str vs Ref?)
        loc_field(F_BRIEF_INFORMATION, "briefInformation"),
        loc_field(F_DETAILED_INFORMATION, "detailedInformation"),
        s(F_LOGO, "logo"),  // TODO(draft): contained MdPicture — уточнить vk
        s(F_SPLASH, "splash"),  // TODO(draft): contained MdPicture — уточнить vk
        loc_field(F_COPYRIGHT, "copyright"),
        loc_field(F_VENDOR_INFORMATION_ADDRESS, "vendorInformationAddress"),
        loc_field(F_CONFIGURATION_INFORMATION_ADDRESS, "configurationInformationAddress"),
        e(F_MAIN_CLIENT_APPLICATION_WINDOW_MODE, "mainClientApplicationWindowMode", "Normal"),  // literals: {Normal|Workplace|FullscreenWorkplace|Kiosk}
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
        e(F_BINARY_DATA_STORAGE_MODE, "binaryDataStorageMode", "DontUse")
            .gated(Since(FormatVersion::new(2, 16))),  // literals: {DontUse|Use}
        e(F_BINARY_DATA_BLOCK_STORAGE_USE_MODE, "binaryDataBlockStorageUseMode", "DontUse")
            .gated(Since(FormatVersion::new(2, 19))),  // literals: {}
        e(F_OBJECT_AUTONUMERATION_MODE, "objectAutonumerationMode", "AutoFree"),  // literals: {AutoFree|NotAutoFree}
        e(F_MODALITY_USE_MODE, "modalityUseMode", "Use"),  // literals: {Use|UseWithWarnings|DontUse}
        e(F_INTERFACE_COMPATIBILITY_MODE, "interfaceCompatibilityMode", "Taxi"),  // literals: {Taxi|TaxiEnableVersion8_2|Version8_2EnableTaxi|Version8_2}
        e(F_VERSION85_INTERFACE_MIGRATION_MODE, "version85InterfaceMigrationMode", "DontUse")
            .gated(Since(FormatVersion::new(2, 21))),  // literals: {DontUse|Use}
        e(F_DATABASE_TABLESPACES_USE_MODE, "databaseTablespacesUseMode", "DontUse")
            .gated(Since(FormatVersion::new(2, 16))),  // literals: {DontUse|Use}
        e(F_COMPATIBILITY_MODE, "compatibilityMode", ""),  // literals: {}
        s(F_DEFAULT_CONSTANTS_FORM, "defaultConstantsForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        s(F_DEFAULT_REPORT_APPEARANCE_TEMPLATE, "defaultReportAppearanceTemplate"),  // TODO(draft): ref -> CommonTemplate (form-ref Str vs Ref?)
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "RequiredMobileApplicationPermissions8315", child_kind: "Configuration.RequiredMobileApplicationPermissions8315" },  // TODO(draft): Ref/stub?
    ChildSlot { collection: "MobileApplicationUrl", child_kind: "Configuration.MobileApplicationUrl" },  // TODO(draft): Ref/stub?
    ChildSlot { collection: "AllowedIncomingShareRequestType", child_kind: "Configuration.AllowedIncomingShareRequestType" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `Configuration` (ЧЕРНОВИК).
pub fn configuration() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Configuration",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `Configuration` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "cf4abeab-37b2-11d4-940f-008048da11f9";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_KEEP_MAPPING_TO_EXTENDED_CONFIGURATION_OBJECTS_BY_I_DS, "47f509d7-3ba3-46c8-8fd8-fbeb3662c0d9"),
    (F_NAME_PREFIX, "52547ac3-1dff-4930-88b8-7419997945ba"),
    (F_CONFIGURATION_EXTENSION_COMPATIBILITY_MODE, "c79cb2e3-6fa1-446a-97f6-68b4742480b5"),
    (F_CONFIGURATION_EXTENSION_PURPOSE, "67363886-e95e-403b-9127-82cf64183fab"),
    (F_DEFAULT_RUN_MODE, "c6c6cdec-9de1-431f-b17a-e442eebf86c1"),
    (F_USE_PURPOSES, "da648ef9-2f12-418f-8e2f-8956bc10a66f"),
    (F_SCRIPT_VARIANT, "2b144fd9-4ab7-4f9e-a5a9-753d43a9840e"),
    (F_DEFAULT_ROLE, "f6cdb7fb-dab4-4699-a22e-feac3e4a2cae"),
    (F_DEFAULT_ROLES, "6a447e3f-d9d7-4c97-a239-87e3fe6d8055"),
    (F_VENDOR, "c5f8bc62-5534-499e-ac1a-d7d043df2a9e"),
    (F_VERSION, "b8c86b25-2e11-4be1-9cd3-9a9cd6ae0f7a"),
    (F_UPDATE_CATALOG_ADDRESS, "b95a55fd-5334-45a9-8189-fdcbb6d71ade"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_USE_MANAGED_FORM_IN_ORDINARY_APPLICATION, "da451f2d-2b84-4c22-a1b4-f3ff02bc87a7"),
    (F_USE_ORDINARY_FORM_IN_MANAGED_APPLICATION, "fe4e2ec0-716b-42ab-a1c4-dfa8dd242b3d"),
    (F_ADDITIONAL_FULL_TEXT_SEARCH_DICTIONARIES, "24ff84da-3683-4c92-8bcd-03f6eb5a208b"),
    (F_COMMON_SETTINGS_STORAGE, "982ff47a-aba8-41b2-aedb-cde5e10291a2"),
    (F_REPORTS_USER_SETTINGS_STORAGE, "932a87cf-144c-449f-970f-19835d547068"),
    (F_REPORTS_VARIANTS_STORAGE, "ce176ad6-af25-4ec7-a660-750772e493ff"),
    (F_FORM_DATA_SETTINGS_STORAGE, "7afdfce2-d83a-4762-b863-06f720de2d86"),
    (F_DYNAMIC_LISTS_USER_SETTINGS_STORAGE, "ce445614-63e1-43ba-910d-72105140e94f"),
    (F_URL_EXTERNAL_DATA_STORAGE, "43a0ac9c-210e-4571-af7e-6ebab49a4dce"),
    (F_CONTENT, "c6690627-40cf-4741-8719-4e7feb832b84"),
    (F_DEFAULT_REPORT_FORM, "3858755b-07b8-4c0b-a7b1-b013192a5863"),
    (F_DEFAULT_REPORT_VARIANT_FORM, "700a5dda-3341-45c2-974e-3c8bfb318228"),
    (F_DEFAULT_REPORT_SETTINGS_FORM, "a78774fa-ae3b-4450-a0d1-22d5654af1bb"),
    (F_DEFAULT_DYNAMIC_LIST_SETTINGS_FORM, "f71e5124-a281-4192-bc51-3be41a230a11"),
    (F_DEFAULT_SEARCH_FORM, "5e3c8a23-7453-4785-a338-2c0d78700e74"),
    (F_DEFAULT_DATA_HISTORY_CHANGE_HISTORY_FORM, "770647c-f103-4c3a-a834-144df18a2938"),
    (F_DEFAULT_DATA_HISTORY_VERSION_DATA_FORM, "5e815a7a-f20f-4a57-a5d0-01c2ceba0dbe"),
    (F_DEFAULT_DATA_HISTORY_VERSION_DIFFERENCES_FORM, "6c487f40-583b-4100-ac4b-7f9ab2678557"),
    (F_DEFAULT_COLLABORATION_SYSTEM_USERS_CHOICE_FORM, "2e69c9c8-7383-438f-a424-29ebaa7dc015"),
    (F_AUXILIARY_REPORT_FORM, "e0fc09aa-cccb-45ec-b0b2-9467af5d05de"),
    (F_AUXILIARY_REPORT_VARIANT_FORM, "ebff9bec-13bb-49de-934b-e9e0a0f859da"),
    (F_AUXILIARY_REPORT_SETTINGS_FORM, "1a0ae3f2-3f27-410b-acba-d9ee82e32c10"),
    (F_AUXILIARY_DYNAMIC_LIST_SETTINGS_FORM, "93303a02-5bd8-4864-bf74-abda2fd24d62"),
    (F_AUXILIARY_DATA_HISTORY_CHANGE_HISTORY_FORM, "7e674d01-addd-41dc-a2a8-527ef98a8dcb"),
    (F_AUXILIARY_DATA_HISTORY_VERSION_DATA_FORM, "f68f24d0-dfa6-4ee9-b4d0-de64ddabf0e4"),
    (F_AUXILIARY_DATA_HISTORY_VERSION_DIFFERENCES_FORM, "f52f3599-b4f1-47be-831c-f10be5b069b2"),
    (F_AUXILIARY_COLLABORATION_SYSTEM_USERS_CHOICE_FORM, "3397beb5-1107-4def-9183-c867cab2d4c0"),
    (F_REQUIRED_MOBILE_APPLICATION_PERMISSIONS, "3431bd92-4a73-4ace-aace-191f9fdeaea2"),
    (F_USED_MOBILE_APPLICATION_FUNCTIONALITIES, "cca11de4-1e73-4988-8c35-2ae49b6c616c"),
    (F_STANDALONE_CONFIGURATION_RESTRICTION_ROLES, "39bfa4e0-334f-428c-803b-9227445f1788"),
    (F_*/, "740eb5f6-e214-4e30-aefa-f15bab91c688"),
    (F_>, "740eb5f6-e214-4e30-aefa-f15bab91c688"),
    (F_CLIENT_APPLICATION_THEME, "ac932464-aba3-4ec5-5d5e-ea639daa8649"),
    (F_MAIN_CLIENT_APPLICATION_WINDOW_INTERFACE_VARIANT, "2fa2e50c-2840-4073-8b50-e6b78d8a5112"),
    (F_CLIENT_APPLICATION_WINDOWS_OPEN_VARIANT, "bf88448f-704c-4fcd-ad70-9321aa09723a"),
    (F_MAIN_SECTION_PICTURE, "6dece861-bb9b-4acd-a35e-5b514531abe1"),
    (F_CAPTION, "4d8ee908-bd04-4026-8060-55223e2e7743"),
    (F_SHORT_CAPTION, "f514ae41-2bf3-4c6b-b1d1-ca69450facbc"),
    (F_DEFAULT_STYLE, "ac8f7099-e6c4-4a11-aa97-26105d03cfbc"),
    (F_DEFAULT_LANGUAGE, "15e3462b-bc9b-40cd-9d9f-dc0922f84560"),
    (F_BRIEF_INFORMATION, "12fce844-eac7-4413-8235-19256a766f21"),
    (F_DETAILED_INFORMATION, "a91a4a6e-0268-4ee2-b48c-9986c8c1dbd9"),
    (F_LOGO, "3035a9db-d6b2-450e-8b22-4430577f8dab"),
    (F_SPLASH, "d98a8e01-7219-41ce-9f23-dada0860b9bf"),
    (F_COPYRIGHT, "876bcf1c-e28d-4749-8619-6138739eb433"),
    (F_VENDOR_INFORMATION_ADDRESS, "cc298508-6a00-422b-a1a0-e2fb933fb408"),
    (F_CONFIGURATION_INFORMATION_ADDRESS, "2efeb7da-4876-464e-9b46-fe7fd5554a8e"),
    (F_MAIN_CLIENT_APPLICATION_WINDOW_MODE, "648d83d8-0468-49a5-a50f-070e7846a231"),
    (F_DATA_LOCK_CONTROL_MODE, "eb610a9d-686b-4e90-8629-8713c914aa2d"),
    (F_BINARY_DATA_STORAGE_MODE, "e20689a6-b18e-43af-8490-49476b8f216e"),
    (F_BINARY_DATA_BLOCK_STORAGE_USE_MODE, "26af5caf-772e-478e-9fe5-156642df6d53"),
    (F_OBJECT_AUTONUMERATION_MODE, "74fc3305-e7f0-4389-918e-f74a105d316c"),
    (F_MODALITY_USE_MODE, "3a5cca5c-0675-43df-80ca-2cf5163335f2"),
    (F_INTERFACE_COMPATIBILITY_MODE, "cbd1f1ed-72d3-4da3-bd02-c643d9595b7d"),
    (F_VERSION85_INTERFACE_MIGRATION_MODE, "5c46b26c-d9b5-4d77-bf05-cdbb7178af13"),
    (F_DATABASE_TABLESPACES_USE_MODE, "9497ea6f-b3d2-433d-ab38-7c562e2932ab"),
    (F_COMPATIBILITY_MODE, "161bc4c0-4cc0-4382-a045-f58f25e24783"),
    (F_DEFAULT_CONSTANTS_FORM, "d4769b90-a0ff-11d5-b99f-0050bae0a95d"),
    (F_DEFAULT_REPORT_APPEARANCE_TEMPLATE, "b73fb5ab-f9e5-4855-8b20-8c1b12f0d41b"),
];
