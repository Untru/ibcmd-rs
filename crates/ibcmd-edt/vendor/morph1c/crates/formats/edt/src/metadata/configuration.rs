//! EDT-проекция КОРНЯ `Configuration` (зеркало `core/spec/metadata/configuration.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! EDT РАЗРЕЖЁННЫЙ: проецирует лишь свойства, которые `.mdo` несёт (synonym, vendor,
//! version, defaultRunMode, …); Designer-only свойства (множество `Default*Form`,
//! storages, window/interface enums) в EDT-карте ОТСУТСТВУЮТ (`lookup→None`) → движок на
//! чтении даёт их канонический дефолт (= Designer-witnessed), на записи не эмитит.
//!
//! Физический порядок EDT-узлов расходится с каноническим (= Designer) порядком спека →
//! [`field_emit_order`] переупорядочивает (сверено по `.mdo`). from-root кодеки
//! (`containedObjects`/`ChildObjects`/`languages`) навигируют от корня (multi-node).

use formats_xml::configuration::ConfigDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::configuration as cfg;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

/// Карта проекции EDT для `Configuration`.
pub struct EdtConfiguration;

impl LocusMap for EdtConfiguration {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            // extension-root свойства (.cfe, witnessed s13_extension): обычный .mdo-корень
            // их не несёт (absent-маркер-дефолты), корень расширения — несёт все шесть.
            cfg::F_OBJECT_BELONGING => fp(&["objectBelonging"], Codec::EnumSparse),
            cfg::F_EXTENSION => fp(&["extension"], Codec::ExtensionFlags),
            cfg::F_KEEP_MAPPING => {
                fp(&["keepMappingToExtendedConfigurationObjectsByIDs"], Codec::BoolPresence)
            }
            cfg::F_NAME_PREFIX => fp(&["namePrefix"], Codec::PlainText),
            // Кодировка канона = EDT-dotted (`8.5.1`) → verbatim EnumText (Designer
            // транслирует, см. Designer-карту / Codec::CompatibilityMode).
            cfg::F_EXT_COMPAT_MODE => {
                fp(&["configurationExtensionCompatibilityMode"], Codec::EnumText)
            }
            cfg::F_EXT_PURPOSE => fp(&["configurationExtensionPurpose"], Codec::EnumSparse),
            cfg::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            // `comment` — EDT .mdo несёт свободный текст (не Designer-only): sparse-опускает
            // при пустом, эмитит иначе. Дефолт "" — X by construction (Designer dense маппит).
            cfg::F_COMMENT => fp(&["comment"], Codec::PlainText),
            cfg::F_CONTAINED_OBJECTS => {
                fp(&["containedObjects"], Codec::ContainedObjects(ConfigDialect::Edt))
            }
            cfg::F_DEFAULT_RUN_MODE => fp(&["defaultRunMode"], Codec::EnumText),
            // usePurposes — EDT: сиблинги `<usePurposes>X</usePurposes>` (plain-text) →
            // `List([Str])` (RefList-Edt). Значения расходятся с Designer → x_ignore в спеке.
            cfg::F_USE_PURPOSES => {
                fp(&["usePurposes"], Codec::RefList(formats_xml::ref_list::RefListDialect::Edt))
            }
            cfg::F_SCRIPT_VARIANT => fp(&["scriptVariant"], Codec::EnumText),
            cfg::F_DEFAULT_ROLES => {
                fp(&["defaultRoles"], Codec::RefList(formats_xml::ref_list::RefListDialect::Edt))
            }
            cfg::F_VENDOR => fp(&["vendor"], Codec::PlainText),
            cfg::F_VERSION => fp(&["version"], Codec::PlainText),
            cfg::F_UPDATE_CATALOG_ADDRESS => fp(&["updateCatalogAddress"], Codec::PlainText),
            cfg::F_INCLUDE_HELP => fp(&["includeHelpInContents"], Codec::BoolPresence),
            cfg::F_HELP => fp(&["help"], Codec::HelpConst),
            cfg::F_SHORT_CAPTION => fp(&["shortCaption"], Codec::LocalizedKeyVal),
            cfg::F_VERSION85_MIGRATION_MODE => fp(&["version85InterfaceMigrationMode"], Codec::EnumText),
            // Считались Designer-only (SSL их не витнессил в .mdo), но ERP-корень несёт все
            // шесть (witness `ERP/edt/src/Configuration/Configuration.mdo`): bool-пара —
            // текст `true` (BoolPresence, как соседний includeHelpInContents; false → EDT
            // опускает = дефолт спека), storage/form-ссылки — PlainText verbatim
            // (`SettingsStorage.ХранилищеВариантовОтчетов`, `CommonForm.ФормаОтчета`, …) —
            // те же значения, что у Designer-зеркала (X by construction).
            cfg::F_USE_MANAGED_FORM_IN_ORDINARY => {
                fp(&["useManagedFormInOrdinaryApplication"], Codec::BoolPresence)
            }
            cfg::F_USE_ORDINARY_FORM_IN_MANAGED => {
                fp(&["useOrdinaryFormInManagedApplication"], Codec::BoolPresence)
            }
            cfg::F_REPORTS_VARIANTS_STORAGE => fp(&["reportsVariantsStorage"], Codec::PlainText),
            cfg::F_DEFAULT_REPORT_FORM => fp(&["defaultReportForm"], Codec::PlainText),
            cfg::F_DEFAULT_REPORT_VARIANT_FORM => {
                fp(&["defaultReportVariantForm"], Codec::PlainText)
            }
            cfg::F_DEFAULT_REPORT_SETTINGS_FORM => {
                fp(&["defaultReportSettingsForm"], Codec::PlainText)
            }
            cfg::F_USED_MOBILE_FUNCTIONALITIES => fp(
                &["usedMobileApplicationFunctionalities"],
                Codec::MobileFunctionalities(ConfigDialect::Edt),
            ),
            cfg::F_ALLOWED_INCOMING_SHARE_TYPES => fp(&["allowedIncomingShareRequestTypes"], Codec::AllowedIncomingShareTypes(ConfigDialect::Edt)),
            cfg::F_WINDOWS_OPEN_VARIANT => fp(&["clientApplicationWindowsOpenVariant"], Codec::EnumText),
            cfg::F_MAIN_SECTION_PICTURE => fp(&["mainSectionPicture"], Codec::PlainText),
            cfg::F_DEFAULT_LANGUAGE => fp(&["defaultLanguage"], Codec::PlainText),
            cfg::F_BRIEF_INFORMATION => fp(&["briefInformation"], Codec::LocalizedKeyVal),
            cfg::F_DETAILED_INFORMATION => fp(&["detailedInformation"], Codec::LocalizedKeyVal),
            cfg::F_SPLASH => fp(&["splash"], Codec::PlainText),
            cfg::F_COPYRIGHT => fp(&["copyright"], Codec::LocalizedKeyVal),
            cfg::F_VENDOR_INFORMATION_ADDRESS => fp(&["vendorInformationAddress"], Codec::LocalizedKeyVal),
            // Localized-пара сиблингов `<configurationInformationAddress><key>ru… /<key>en…`
            // (witness ERP Configuration.mdo: ×2 языка, сразу после vendorInformationAddress —
            // как в metamodel; SSL узла не нёс).
            cfg::F_CONFIGURATION_INFORMATION_ADDRESS => {
                fp(&["configurationInformationAddress"], Codec::LocalizedKeyVal)
            }
            cfg::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            cfg::F_OBJECT_AUTONUMERATION_MODE => fp(&["objectAutonumerationMode"], Codec::EnumText),
            cfg::F_MODALITY_USE_MODE => fp(&["modalityUseMode"], Codec::EnumText),
            // EDT ЭМИТИТ это свойство, когда его значение != EDT-implicit-дефолт `Use`
            // (min_const: `DontUse`); SSL опускает (значение == `Use`). Проецируем в EDT
            // (не «Designer-only»): движок читает present-значение, absent → канон-дефолт
            // `Use`, byte-exact на записи (SSL sparse-опускает `Use`, min_const эмитит).
            cfg::F_SYNC_PLATFORM_CALL_USE_MODE => {
                fp(&["synchronousPlatformExtensionAndAddInCallUseMode"], Codec::EnumText)
            }
            cfg::F_INTERFACE_COMPATIBILITY_MODE => fp(&["interfaceCompatibilityMode"], Codec::EnumText),
            cfg::F_COMPATIBILITY_MODE => fp(&["compatibilityMode"], Codec::EnumText),
            cfg::F_LANGUAGES => fp(&["languages"], Codec::LanguagesEntity),
            cfg::F_CHILD_OBJECTS => fp(&["ChildObjects"], Codec::ConfigChildObjects(ConfigDialect::Edt)),
            // Designer-only свойства EDT не проецирует (lookup→None → канон-дефолт).
            _ => return None,
        })
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(EDT_ORDER)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // Корень РАСШИРЕНИЯ (.cfe, s13_extension) несёт xmlns:xsi (его требует узел
        // `<extension xsi:type="…">`). Обычный корень его НЕ несёт: чтение claim'ит лишь
        // присутствующие (declare-iff-used), запись объявляет РОВНО используемые телом.
        &[("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")]
    }

    fn root_extra_namespaces_trailing(&self) -> &'static [(&'static str, &'static str)] {
        // xmlns:mdclassExtension корня расширения идёт ПОСЛЕ xmlns:mdclass (witnessed
        // s13: `xsi, mdclass, mdclassExtension`) — хвостовая позиция, тот же declare-iff-used.
        &[("xmlns:mdclassExtension", "http://g5.1c.ru/v8/dt/metadata/mdclass/extension")]
    }
}

/// WRITE-карта проекции EDT для корня РАСШИРЕНИЯ: та же раскладка, что у
/// [`EdtConfiguration`], НО `configurationExtensionCompatibilityMode` эмитится ВСЕГДА
/// (witnessed: extension-`.mdo` несёт его и при значении == канон-дефолт `8.5.1`,
/// который обычный sparse-корень опускает). Достигается пер-форматным дефолтом-сентинелом:
/// re-sparsify на записи сверяет значение с ним и никогда не совпадает → эмиссия.
/// Чтение обоих shape ведёт базовая карта; выбор — [`write`] по
/// [`morph1c_core::spec::metadata::configuration::is_extension_root`].
pub struct EdtConfigurationExtension;

impl LocusMap for EdtConfigurationExtension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        EdtConfiguration.lookup(field)
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        EdtConfiguration.field_emit_order()
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        EdtConfiguration.root_extra_namespaces()
    }

    fn root_extra_namespaces_trailing(&self) -> &'static [(&'static str, &'static str)] {
        EdtConfiguration.root_extra_namespaces_trailing()
    }

    fn field_default(&self, field: FieldId) -> Option<morph1c_core::ir::value::PropertyValue> {
        use morph1c_core::ir::value::{PropertyValue, Token};
        match field {
            // Сентинел, не равный ни одному реальному литералу → is_default всегда false →
            // sparse-писатель эмитит поле при ЛЮБОМ значении (включая канон-дефолт `8.5.1`).
            cfg::F_EXT_COMPAT_MODE => Some(PropertyValue::Enum(Token::new("\u{0}"))),
            _ => EdtConfiguration.field_default(field),
        }
    }
}

/// Физический порядок EDT-узлов (сверено по `Configuration.mdo`). Только EDT-эмитируемые
/// поля; Designer-only — отсутствуют (их движок и не эмитит в EDT).
static EDT_ORDER: &[FieldId] = &[
    cfg::F_SYNONYM,
    cfg::F_COMMENT,
    // extension-root регион (.cfe, сверено s13_extension/Configuration.mdo): objectBelonging
    // + extension — ДО containedObjects; keepMapping…/namePrefix/extCompatMode/extPurpose —
    // ПОСЛЕ containedObjects, до defaultRunMode.
    cfg::F_OBJECT_BELONGING,
    cfg::F_EXTENSION,
    cfg::F_CONTAINED_OBJECTS,
    cfg::F_KEEP_MAPPING,
    cfg::F_NAME_PREFIX,
    cfg::F_EXT_COMPAT_MODE,
    cfg::F_EXT_PURPOSE,
    cfg::F_DEFAULT_RUN_MODE,
    cfg::F_USE_PURPOSES,
    cfg::F_SCRIPT_VARIANT,
    cfg::F_DEFAULT_ROLES,
    cfg::F_VENDOR,
    cfg::F_VERSION,
    cfg::F_UPDATE_CATALOG_ADDRESS,
    cfg::F_INCLUDE_HELP,
    cfg::F_HELP,
    // ERP-witnessed блок (Configuration.mdo:29–34, порядок = metamodel: includeHelpInContents →
    // useManagedForm… → useOrdinaryForm… → reportsVariantsStorage → defaultReport*Form →
    // usedMobileApplicationFunctionalities).
    cfg::F_USE_MANAGED_FORM_IN_ORDINARY,
    cfg::F_USE_ORDINARY_FORM_IN_MANAGED,
    cfg::F_REPORTS_VARIANTS_STORAGE,
    cfg::F_DEFAULT_REPORT_FORM,
    cfg::F_DEFAULT_REPORT_VARIANT_FORM,
    cfg::F_DEFAULT_REPORT_SETTINGS_FORM,
    cfg::F_USED_MOBILE_FUNCTIONALITIES,
    cfg::F_ALLOWED_INCOMING_SHARE_TYPES,
    cfg::F_WINDOWS_OPEN_VARIANT,
    cfg::F_MAIN_SECTION_PICTURE,
    cfg::F_SHORT_CAPTION,
    cfg::F_DEFAULT_LANGUAGE,
    cfg::F_BRIEF_INFORMATION,
    cfg::F_DETAILED_INFORMATION,
    cfg::F_SPLASH,
    cfg::F_COPYRIGHT,
    cfg::F_VENDOR_INFORMATION_ADDRESS,
    // ERP-witnessed (Configuration.mdo:111–118): сразу после vendorInformationAddress.
    cfg::F_CONFIGURATION_INFORMATION_ADDRESS,
    cfg::F_DATA_LOCK_CONTROL_MODE,
    cfg::F_OBJECT_AUTONUMERATION_MODE,
    cfg::F_MODALITY_USE_MODE,
    cfg::F_SYNC_PLATFORM_CALL_USE_MODE,
    cfg::F_INTERFACE_COMPATIBILITY_MODE,
    cfg::F_VERSION85_MIGRATION_MODE,
    cfg::F_COMPATIBILITY_MODE,
    cfg::F_LANGUAGES,
    cfg::F_CHILD_OBJECTS,
];

// --- Строка R+X-харнесса ---
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::configuration::{
    configuration, is_extension_root, EXTENSION_WITNESSED_FLAGS,
};

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Configuration", configuration(), &EdtConfiguration, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    if is_extension_root(&obj.properties) {
        let mut ext = obj.clone();
        // `<extension>`-блок — EDT-only (Designer-дескриптор аналога не несёт, поле
        // x_ignore), поэтому designer→edt СИНТЕЗИРУЕТ witnessed-набор Checked-флагов
        // (единственная засвидетельствованная форма, s13). EDT-источник несёт свои
        // флаги в bag — они round-trip'ятся verbatim, синтез не срабатывает.
        if !ext.properties.iter().any(|(id, _)| *id == cfg::F_EXTENSION) {
            ext.properties.push((
                cfg::F_EXTENSION,
                PropertyValue::List(
                    EXTENSION_WITNESSED_FLAGS
                        .iter()
                        .map(|f| PropertyValue::Str((*f).to_string()))
                        .collect(),
                ),
            ));
        }
        // Witnessed extension-`.mdo` НЕ несёт пустых узлов `<mainSectionPicture/>`/
        // `<splash/>` (те существуют лишь при наличии картинок; presence гейтится файлами,
        // см. `pipeline::ext_read`). Present-empty `Str("")` в bag корня расширения
        // (EDT-источник с узлом либо pipeline-синтез из Ext-картинки — оба unwitnessed
        // для .cfe) снимаем молча; НЕПУСТОЕ значение (не witnessed) — громко.
        for fid in [cfg::F_MAIN_SECTION_PICTURE, cfg::F_SPLASH] {
            if let Some(pos) = ext.properties.iter().position(|(id, _)| *id == fid) {
                match &ext.properties[pos].1 {
                    PropertyValue::Str(s) if s.is_empty() => {
                        ext.properties.remove(pos);
                    }
                    other => {
                        return Err(format!(
                            "extension root carries a non-empty picture value {other:?} for \
                             field {fid:?} — not part of the witnessed extension-root shape \
                             (§1.0 — refusing to emit it silently)"
                        ))
                    }
                }
            }
        }
        return crate::write_descriptor(
            "Configuration",
            configuration(),
            &EdtConfigurationExtension,
            &ext,
        )
        .map_err(|e| e.to_string());
    }
    crate::write_descriptor("Configuration", configuration(), &EdtConfiguration, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Configuration. Корень — singleton (`Configuration.mdo`).
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Configuration",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/Configuration",
    layout: formats_xml::CorpusLayout::SingletonFile { file: "Configuration.mdo", name: "Configuration" },
};
