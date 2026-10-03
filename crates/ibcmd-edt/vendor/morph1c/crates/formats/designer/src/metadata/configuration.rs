//! Designer-проекция КОРНЯ `Configuration` (зеркало `core/spec/metadata/configuration.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Designer ПЛОТНЫЙ (`emit_defaults=true`): проецирует и эмитит ВСЕ ~71 свойств в
//! каноническом (= спек) порядке, в т.ч. дефолтные (`<Comment/>`, `<DefaultReportForm/>`,
//! `<CompatibilityMode>Version8_5_1</…>`). Локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<Configuration>/<Properties>`. from-root кодеки (`containedObjects` → `<InternalInfo>`,
//! `ChildObjects`) навигируют от `<MetaDataObject>` сами (descend к `<Configuration>`).

use formats_xml::configuration::ConfigDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::configuration as cfg;

/// Построить `FieldProjection` с локусом-путём от корня `<MetaDataObject>`.
const fn p(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: XmlLocus::PropElement { path, ns: "" }, codec }
}

// Пути от корня (`<MetaDataObject>`) к каждому property-тегу. Объявлены статически.
macro_rules! prop_path {
    ($name:ident, $tag:literal) => {
        const $name: &[&str] = &["Configuration", "Properties", $tag];
    };
}

prop_path!(PP_SYNONYM, "Synonym");
prop_path!(PP_COMMENT, "Comment");
// extension-root свойства (.cfe, witnessed s13_extension; ОТСУТСТВУЮТ у обычного dense-корня
// s1..s12 → разрежённые кодеки EnumSparse/BoolPresence, absent-маркер-дефолты).
prop_path!(PP_OBJECT_BELONGING, "ObjectBelonging");
prop_path!(PP_EXT_PURPOSE, "ConfigurationExtensionPurpose");
prop_path!(PP_KEEP_MAPPING, "KeepMappingToExtendedConfigurationObjectsByIDs");
prop_path!(PP_NAME_PREFIX, "NamePrefix");
prop_path!(PP_EXT_COMPAT, "ConfigurationExtensionCompatibilityMode");
prop_path!(PP_DEFAULT_RUN_MODE, "DefaultRunMode");
prop_path!(PP_USE_PURPOSES, "UsePurposes");
prop_path!(PP_SCRIPT_VARIANT, "ScriptVariant");
prop_path!(PP_DEFAULT_ROLES, "DefaultRoles");
prop_path!(PP_VENDOR, "Vendor");
prop_path!(PP_VERSION, "Version");
prop_path!(PP_UPDATE_CATALOG, "UpdateCatalogAddress");
prop_path!(PP_INCLUDE_HELP, "IncludeHelpInContents");
prop_path!(PP_USE_MANAGED_ORD, "UseManagedFormInOrdinaryApplication");
prop_path!(PP_USE_ORD_MANAGED, "UseOrdinaryFormInManagedApplication");
prop_path!(PP_ADD_FTS_DICTS, "AdditionalFullTextSearchDictionaries");
prop_path!(PP_COMMON_SETTINGS_STORAGE, "CommonSettingsStorage");
prop_path!(PP_REPORTS_USER_SETTINGS, "ReportsUserSettingsStorage");
prop_path!(PP_REPORTS_VARIANTS, "ReportsVariantsStorage");
prop_path!(PP_FORM_DATA_SETTINGS, "FormDataSettingsStorage");
prop_path!(PP_DYN_LISTS_USER_SETTINGS, "DynamicListsUserSettingsStorage");
prop_path!(PP_URL_EXT_DATA, "URLExternalDataStorage");
prop_path!(PP_CONTENT, "Content");
prop_path!(PP_DEFAULT_REPORT_FORM, "DefaultReportForm");
prop_path!(PP_DEFAULT_REPORT_VARIANT, "DefaultReportVariantForm");
prop_path!(PP_DEFAULT_REPORT_SETTINGS, "DefaultReportSettingsForm");
prop_path!(PP_DEFAULT_REPORT_APPEARANCE, "DefaultReportAppearanceTemplate");
prop_path!(PP_DEFAULT_DYN_LIST_SETTINGS, "DefaultDynamicListSettingsForm");
prop_path!(PP_DEFAULT_SEARCH, "DefaultSearchForm");
prop_path!(PP_DEFAULT_DH_CHANGE, "DefaultDataHistoryChangeHistoryForm");
prop_path!(PP_DEFAULT_DH_VER_DATA, "DefaultDataHistoryVersionDataForm");
prop_path!(PP_DEFAULT_DH_VER_DIFF, "DefaultDataHistoryVersionDifferencesForm");
prop_path!(PP_DEFAULT_COLLAB, "DefaultCollaborationSystemUsersChoiceForm");
prop_path!(PP_AUX_REPORT_FORM, "AuxiliaryReportForm");
prop_path!(PP_AUX_REPORT_VARIANT, "AuxiliaryReportVariantForm");
prop_path!(PP_AUX_REPORT_SETTINGS, "AuxiliaryReportSettingsForm");
prop_path!(PP_AUX_DYN_LIST_SETTINGS, "AuxiliaryDynamicListSettingsForm");
prop_path!(PP_AUX_DH_CHANGE, "AuxiliaryDataHistoryChangeHistoryForm");
prop_path!(PP_AUX_DH_VER_DATA, "AuxiliaryDataHistoryVersionDataForm");
prop_path!(PP_AUX_DH_VER_DIFF, "AuxiliaryDataHistoryVersionDifferencesForm");
prop_path!(PP_AUX_COLLAB, "AuxiliaryCollaborationSystemUsersChoiceForm");
prop_path!(PP_REQ_MOBILE_PERMS, "RequiredMobileApplicationPermissions");
prop_path!(PP_USED_MOBILE_FUNC, "UsedMobileApplicationFunctionalities");
prop_path!(PP_STANDALONE_ROLES, "StandaloneConfigurationRestrictionRoles");
prop_path!(PP_MOBILE_URLS, "MobileApplicationURLs");
prop_path!(PP_ALLOWED_SHARE, "AllowedIncomingShareRequestTypes");
prop_path!(PP_MAIN_WIN_INTERFACE, "MainClientApplicationWindowInterfaceVariant");
prop_path!(PP_CLIENT_THEME, "ClientApplicationTheme");
prop_path!(PP_MAIN_WIN_MODE, "MainClientApplicationWindowMode");
prop_path!(PP_WINDOWS_OPEN, "ClientApplicationWindowsOpenVariant");
prop_path!(PP_DEFAULT_INTERFACE, "DefaultInterface");
prop_path!(PP_CAPTION, "Caption");
prop_path!(PP_SHORT_CAPTION, "ShortCaption");
prop_path!(PP_DEFAULT_STYLE, "DefaultStyle");
prop_path!(PP_DEFAULT_LANGUAGE, "DefaultLanguage");
prop_path!(PP_BRIEF_INFO, "BriefInformation");
prop_path!(PP_DETAILED_INFO, "DetailedInformation");
prop_path!(PP_COPYRIGHT, "Copyright");
prop_path!(PP_VENDOR_INFO_ADDR, "VendorInformationAddress");
prop_path!(PP_CONFIG_INFO_ADDR, "ConfigurationInformationAddress");
prop_path!(PP_DATA_LOCK, "DataLockControlMode");
prop_path!(PP_OBJ_AUTONUM, "ObjectAutonumerationMode");
prop_path!(PP_MODALITY, "ModalityUseMode");
prop_path!(PP_SYNC_PLATFORM, "SynchronousPlatformExtensionAndAddInCallUseMode");
prop_path!(PP_INTERFACE_COMPAT, "InterfaceCompatibilityMode");
prop_path!(PP_VER85_MIGRATION, "Version85InterfaceMigrationMode");
prop_path!(PP_DB_TABLESPACES, "DatabaseTablespacesUseMode");
prop_path!(PP_COMPAT_MODE, "CompatibilityMode");
prop_path!(PP_DEFAULT_CONSTANTS_FORM, "DefaultConstantsForm");

// ChildObjects/InternalInfo — from-root кодеки (навигируют сами); локус-путь только для
// `field_emit_order`-нейтральности (декод не использует путь).
prop_path!(PP_CHILD_OBJECTS, "ChildObjects");
prop_path!(PP_CONTAINED, "InternalInfo");

const fn loc_kv(path: &'static [&'static str]) -> FieldProjection {
    p(path, Codec::LocalizedV8)
}
const fn enum_t(path: &'static [&'static str]) -> FieldProjection {
    p(path, Codec::EnumText)
}
const fn plain(path: &'static [&'static str]) -> FieldProjection {
    p(path, Codec::PlainText)
}
const fn bool_t(path: &'static [&'static str]) -> FieldProjection {
    p(path, Codec::BoolText)
}

/// Карта проекции Designer для `Configuration`.
pub struct DesignerConfiguration;

impl LocusMap for DesignerConfiguration {
    fn field_emit_order_for_version(
        &self,
        target: morph1c_core::version::FormatVersion,
    ) -> Option<std::borrow::Cow<'static, [FieldId]>> {
        if target != morph1c_core::version::FormatVersion::new(2, 21) {
            return self.field_emit_order().map(std::borrow::Cow::Borrowed);
        }
        // Native 8.5 materializes Interface, Theme, Mode, WindowsOpen in this
        // order. The EDT exporter retains the specification order instead.
        // Reposition only these scalar fields; every current value is emitted
        // by the ordinary typed projection, with repeated contents untouched.
        let mut order = Vec::new();
        for field in cfg::configuration().fields() {
            if matches!(
                field.id,
                cfg::F_MAIN_WINDOW_MODE | cfg::F_CLIENT_APPLICATION_THEME
            ) {
                continue;
            }
            order.push(field.id);
            if field.id == cfg::F_MAIN_WINDOW_INTERFACE_VARIANT {
                order.push(cfg::F_CLIENT_APPLICATION_THEME);
                order.push(cfg::F_MAIN_WINDOW_MODE);
            }
        }
        Some(std::borrow::Cow::Owned(order))
    }

    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cfg::F_SYNONYM => loc_kv(PP_SYNONYM),
            cfg::F_CONTAINED_OBJECTS => p(PP_CONTAINED, Codec::ContainedObjects(ConfigDialect::Designer)),
            cfg::F_COMMENT => plain(PP_COMMENT),
            // extension-root свойства: present ТОЛЬКО на корне расширения (EnumSparse/
            // BoolPresence не эмитят absent-маркер/false → обычный dense-корень их не несёт).
            cfg::F_OBJECT_BELONGING => p(PP_OBJECT_BELONGING, Codec::EnumSparse),
            cfg::F_EXT_PURPOSE => p(PP_EXT_PURPOSE, Codec::EnumSparse),
            cfg::F_KEEP_MAPPING => p(PP_KEEP_MAPPING, Codec::BoolPresence),
            cfg::F_NAME_PREFIX => plain(PP_NAME_PREFIX),
            // Кодировка расходится: Designer `Version8_5_1` ↔ канон/EDT `8.5.1` — транслирующий
            // кодек (как compatibilityMode ниже). Дефолт спека = канон `8.5.1` → dense-эмиссия
            // дефолта даёт byte-exact `Version8_5_1` на обычном корне.
            cfg::F_EXT_COMPAT_MODE => p(PP_EXT_COMPAT, Codec::CompatibilityMode),
            cfg::F_DEFAULT_RUN_MODE => enum_t(PP_DEFAULT_RUN_MODE),
            cfg::F_USE_PURPOSES => p(PP_USE_PURPOSES, Codec::UsePurposesV8),
            cfg::F_SCRIPT_VARIANT => enum_t(PP_SCRIPT_VARIANT),
            cfg::F_DEFAULT_ROLES => {
                p(PP_DEFAULT_ROLES, Codec::RefList(formats_xml::ref_list::RefListDialect::DesignerItem))
            }
            cfg::F_VENDOR => plain(PP_VENDOR),
            cfg::F_VERSION => plain(PP_VERSION),
            cfg::F_UPDATE_CATALOG_ADDRESS => plain(PP_UPDATE_CATALOG),
            cfg::F_INCLUDE_HELP => bool_t(PP_INCLUDE_HELP),
            cfg::F_USE_MANAGED_FORM_IN_ORDINARY => bool_t(PP_USE_MANAGED_ORD),
            cfg::F_USE_ORDINARY_FORM_IN_MANAGED => bool_t(PP_USE_ORD_MANAGED),
            cfg::F_ADDITIONAL_FTS_DICTS => plain(PP_ADD_FTS_DICTS),
            cfg::F_COMMON_SETTINGS_STORAGE => plain(PP_COMMON_SETTINGS_STORAGE),
            cfg::F_REPORTS_USER_SETTINGS_STORAGE => plain(PP_REPORTS_USER_SETTINGS),
            cfg::F_REPORTS_VARIANTS_STORAGE => plain(PP_REPORTS_VARIANTS),
            cfg::F_FORM_DATA_SETTINGS_STORAGE => plain(PP_FORM_DATA_SETTINGS),
            cfg::F_DYNAMIC_LISTS_USER_SETTINGS_STORAGE => plain(PP_DYN_LISTS_USER_SETTINGS),
            cfg::F_URL_EXTERNAL_DATA_STORAGE => plain(PP_URL_EXT_DATA),
            cfg::F_CONTENT => plain(PP_CONTENT),
            cfg::F_DEFAULT_REPORT_FORM => plain(PP_DEFAULT_REPORT_FORM),
            cfg::F_DEFAULT_REPORT_VARIANT_FORM => plain(PP_DEFAULT_REPORT_VARIANT),
            cfg::F_DEFAULT_REPORT_SETTINGS_FORM => plain(PP_DEFAULT_REPORT_SETTINGS),
            cfg::F_DEFAULT_REPORT_APPEARANCE_TEMPLATE => plain(PP_DEFAULT_REPORT_APPEARANCE),
            cfg::F_DEFAULT_DYNAMIC_LIST_SETTINGS_FORM => plain(PP_DEFAULT_DYN_LIST_SETTINGS),
            cfg::F_DEFAULT_SEARCH_FORM => plain(PP_DEFAULT_SEARCH),
            cfg::F_DEFAULT_DH_CHANGE_HISTORY_FORM => plain(PP_DEFAULT_DH_CHANGE),
            cfg::F_DEFAULT_DH_VERSION_DATA_FORM => plain(PP_DEFAULT_DH_VER_DATA),
            cfg::F_DEFAULT_DH_VERSION_DIFF_FORM => plain(PP_DEFAULT_DH_VER_DIFF),
            cfg::F_DEFAULT_COLLAB_USERS_CHOICE_FORM => plain(PP_DEFAULT_COLLAB),
            cfg::F_AUX_REPORT_FORM => plain(PP_AUX_REPORT_FORM),
            cfg::F_AUX_REPORT_VARIANT_FORM => plain(PP_AUX_REPORT_VARIANT),
            cfg::F_AUX_REPORT_SETTINGS_FORM => plain(PP_AUX_REPORT_SETTINGS),
            cfg::F_AUX_DYNAMIC_LIST_SETTINGS_FORM => plain(PP_AUX_DYN_LIST_SETTINGS),
            cfg::F_AUX_DH_CHANGE_HISTORY_FORM => plain(PP_AUX_DH_CHANGE),
            cfg::F_AUX_DH_VERSION_DATA_FORM => plain(PP_AUX_DH_VER_DATA),
            cfg::F_AUX_DH_VERSION_DIFF_FORM => plain(PP_AUX_DH_VER_DIFF),
            cfg::F_AUX_COLLAB_USERS_CHOICE_FORM => plain(PP_AUX_COLLAB),
            cfg::F_REQUIRED_MOBILE_PERMISSIONS => plain(PP_REQ_MOBILE_PERMS),
            cfg::F_USED_MOBILE_FUNCTIONALITIES => {
                p(PP_USED_MOBILE_FUNC, Codec::MobileFunctionalities(ConfigDialect::Designer))
            }
            cfg::F_STANDALONE_RESTRICTION_ROLES => plain(PP_STANDALONE_ROLES),
            cfg::F_MOBILE_APPLICATION_URLS => plain(PP_MOBILE_URLS),
            cfg::F_ALLOWED_INCOMING_SHARE_TYPES => p(PP_ALLOWED_SHARE, Codec::AllowedIncomingShareTypes(ConfigDialect::Designer)),
            cfg::F_MAIN_WINDOW_INTERFACE_VARIANT => enum_t(PP_MAIN_WIN_INTERFACE),
            cfg::F_CLIENT_APPLICATION_THEME => enum_t(PP_CLIENT_THEME),
            cfg::F_MAIN_WINDOW_MODE => enum_t(PP_MAIN_WIN_MODE),
            cfg::F_WINDOWS_OPEN_VARIANT => enum_t(PP_WINDOWS_OPEN),
            // mainSectionPicture: EDT-only — Designer не проецирует (узла в дескрипторе
            // нет ВООБЩЕ; presence гейтится файлом картинки Ext/MainSectionPicture.xml —
            // present-empty `Str("")` синтезирует pipeline::ext_read при attach).
            cfg::F_MAIN_SECTION_PICTURE => return None,
            cfg::F_DEFAULT_INTERFACE => plain(PP_DEFAULT_INTERFACE),
            cfg::F_CAPTION => loc_kv(PP_CAPTION),
            cfg::F_SHORT_CAPTION => loc_kv(PP_SHORT_CAPTION),
            cfg::F_DEFAULT_STYLE => plain(PP_DEFAULT_STYLE),
            cfg::F_DEFAULT_LANGUAGE => plain(PP_DEFAULT_LANGUAGE),
            cfg::F_BRIEF_INFORMATION => loc_kv(PP_BRIEF_INFO),
            cfg::F_DETAILED_INFORMATION => loc_kv(PP_DETAILED_INFO),
            // splash: EDT-only — Designer не проецирует (как mainSectionPicture: presence
            // гейтится файлом Ext/Splash.xml, синтез — pipeline::ext_read).
            cfg::F_SPLASH | cfg::F_LOGO => return None,
            cfg::F_COPYRIGHT => loc_kv(PP_COPYRIGHT),
            cfg::F_VENDOR_INFORMATION_ADDRESS => loc_kv(PP_VENDOR_INFO_ADDR),
            cfg::F_CONFIGURATION_INFORMATION_ADDRESS => loc_kv(PP_CONFIG_INFO_ADDR),
            cfg::F_DATA_LOCK_CONTROL_MODE => enum_t(PP_DATA_LOCK),
            cfg::F_OBJECT_AUTONUMERATION_MODE => enum_t(PP_OBJ_AUTONUM),
            cfg::F_MODALITY_USE_MODE => enum_t(PP_MODALITY),
            cfg::F_SYNC_PLATFORM_CALL_USE_MODE => enum_t(PP_SYNC_PLATFORM),
            cfg::F_INTERFACE_COMPATIBILITY_MODE => enum_t(PP_INTERFACE_COMPAT),
            cfg::F_VERSION85_MIGRATION_MODE => enum_t(PP_VER85_MIGRATION),
            cfg::F_DATABASE_TABLESPACES_USE_MODE => enum_t(PP_DB_TABLESPACES),
            // Режим совместимости кодируется РАСХОДЯЩЕ: Designer `Version8_3_20` ↔ канон/EDT
            // `8.3.20`. Спец-кодек транслирует read/write (edt→designer иначе эмитит `8.3.20`,
            // которое платформа отвергает). См. `formats_xml::Codec::CompatibilityMode`.
            cfg::F_COMPATIBILITY_MODE => p(PP_COMPAT_MODE, Codec::CompatibilityMode),
            cfg::F_DEFAULT_CONSTANTS_FORM => plain(PP_DEFAULT_CONSTANTS_FORM),
            // languages: EDT-only — Designer тело языка в отдельном файле.
            cfg::F_LANGUAGES => return None,
            cfg::F_CHILD_OBJECTS => p(PP_CHILD_OBJECTS, Codec::ConfigChildObjects(ConfigDialect::Designer)),
            _ => return None,
        })
    }

    // NB mainSectionPicture/splash: НИКАКОГО пер-форматного дефолта. EDT несёт пустой узел
    // (`<mainSectionPicture/>`/`<splash/>`) РОВНО когда картинка существует (witnessed:
    // s15 — оба узла + Ext-файлы; s1..s14 — ни узлов, ни файлов); Designer узла не несёт
    // вовсе. Designer-absent → канон-sentinel → сжимается из bag; present-empty `Str("")`
    // при наличии картинки синтезирует `pipeline::ext_read::attach_config_ext` — presence
    // гейтится файлами, не дескриптором.
}

// --- Extension-root (.cfe) shape ---

/// Witnessed-набор полей КОРНЯ РАСШИРЕНИЯ в Designer-дескрипторе (спек-порядок ==
/// физический порядок coverage/designer/s13_extension/Configuration.xml). Полный dense-
/// корень (.cf, ~71 свойство) сюда НЕ входит: платформа эмитит для расширения РОВНО этот
/// усечённый набор, а лишние свойства (в т.ч. ПУСТОЙ enum `ClientApplicationWindowsOpen
/// Variant`) валят XDTO-загрузку при cfe_compile.
const EXT_ROOT_FIELDS: &[FieldId] = &[
    cfg::F_OBJECT_BELONGING,
    cfg::F_SYNONYM,
    cfg::F_CONTAINED_OBJECTS,
    cfg::F_COMMENT,
    cfg::F_EXT_PURPOSE,
    cfg::F_KEEP_MAPPING,
    cfg::F_NAME_PREFIX,
    cfg::F_EXT_COMPAT_MODE,
    cfg::F_DEFAULT_RUN_MODE,
    cfg::F_USE_PURPOSES,
    cfg::F_SCRIPT_VARIANT,
    cfg::F_VENDOR,
    cfg::F_VERSION,
    cfg::F_CAPTION,
    cfg::F_SHORT_CAPTION,
    cfg::F_DEFAULT_LANGUAGE,
    cfg::F_BRIEF_INFORMATION,
    cfg::F_DETAILED_INFORMATION,
    cfg::F_COPYRIGHT,
    cfg::F_VENDOR_INFORMATION_ADDRESS,
    cfg::F_CONFIGURATION_INFORMATION_ADDRESS,
    cfg::F_MODALITY_USE_MODE,
    cfg::F_SYNC_PLATFORM_CALL_USE_MODE,
    cfg::F_INTERFACE_COMPATIBILITY_MODE,
    cfg::F_COMPATIBILITY_MODE,
    cfg::F_CHILD_OBJECTS,
];

/// WRITE-карта проекции Designer для корня РАСШИРЕНИЯ: проецирует РОВНО witnessed-набор
/// [`EXT_ROOT_FIELDS`] (кодеки/локусы — те же, что у полной карты), поэтому dense-эмиссия
/// воспроизводит усечённый extension-корень byte-exact. `ObjectBelonging` — ПЕРЕД `<Name>`
/// (witnessed-позиция). Чтение обоих shape ведёт ПОЛНАЯ карта (позиционно-независимо,
/// absent → дефолт) — эта нужна только записи; выбор — [`write`] по
/// [`morph1c_core::spec::metadata::configuration::is_extension_root`].
pub struct DesignerConfigurationExtension;

impl LocusMap for DesignerConfigurationExtension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if EXT_ROOT_FIELDS.contains(&field) {
            DesignerConfiguration.lookup(field)
        } else {
            None
        }
    }

    fn properties_head_fields(&self) -> &'static [FieldId] {
        &[cfg::F_OBJECT_BELONGING]
    }
}

// --- Строка R+X-харнесса ---
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::configuration::{configuration, is_extension_root};

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Configuration", configuration(), &DesignerConfiguration, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    if is_extension_root(&obj.properties) {
        // §1.0: свойство источника, которое Designer СПОСОБЕН нести (полная карта его
        // проецирует), но witnessed extension-корень НЕ несёт, — громкий отказ, не
        // молчаливый дроп значения. Поля вне полной карты (EDT-only `extension`/
        // `languages`/`mainSectionPicture`/`splash`) legitимно не эмитятся и тут.
        for (id, _) in &obj.properties {
            if DesignerConfiguration.lookup(*id).is_some() && !EXT_ROOT_FIELDS.contains(id) {
                let name = configuration()
                    .fields()
                    .iter()
                    .find(|f| f.id == *id)
                    .map(|f| f.name)
                    .unwrap_or("?");
                return Err(format!(
                    "extension root carries property {name:?} which is not part of the \
                     witnessed Designer extension-root shape (§1.0 — refusing to emit it \
                     silently; witnessed: coverage/designer/s13_extension)"
                ));
            }
        }
        return crate::write_descriptor(
            "Configuration",
            configuration(),
            &DesignerConfigurationExtension,
            obj,
        )
        .map_err(|e| e.to_string());
    }
    crate::write_descriptor("Configuration", configuration(), &DesignerConfiguration, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Configuration. Корень — singleton (`Configuration.xml`).
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Configuration",
    read,
    write,
    corpus_subpath: "coverage/designer/s1_core",
    layout: formats_xml::CorpusLayout::SingletonFile { file: "Configuration.xml", name: "Configuration" },
};
