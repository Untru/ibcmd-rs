//! EDT-проекция вида `SettingsStorage` + его дочерних видов (зеркало
//! `core/spec/metadata/settings_storage*.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Переиспользует СУЩЕСТВУЮЩИЙ
//! субстрат: FormRef/TemplateRef inline-стабы, `HelpConst`, `RefList`-Edt (переменный
//! `usePurposes` стаба — 1..2 сиблинга `<usePurposes>X</usePurposes>`, сверено 13/13).
//! Платформенный `<producedTypes>` (Manager) фреймится каркасом (`PRODUCED_CATEGORIES`).
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo` (sparse; физический порядок present-полей ==
//! спек-порядку: synonym, defaultSaveForm, defaultLoadForm — сверено 1/1 SSL).
//! Forms/Templates — inline-СТАБЫ (`bare_ref=false`). Тело `ManagerModule.bsl` — сайдкар,
//! вне дескриптора.

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;

use morph1c_core::spec::metadata::settings_storage as ss;
use morph1c_core::spec::metadata::settings_storage::settings_storage;
use morph1c_core::spec::metadata::settings_storage_form_ref as fref;
use morph1c_core::spec::metadata::settings_storage_form_ref::settings_storage_form_ref;
use morph1c_core::spec::metadata::settings_storage_template_ref as tref;
use morph1c_core::spec::metadata::settings_storage_template_ref::settings_storage_template_ref;

const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ SettingsStorage =====
pub struct EdtSettingsStorage;

impl LocusMap for EdtSettingsStorage {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ss::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            ss::F_COMMENT => fp(&["comment"], Codec::PlainText),
            ss::F_DEFAULT_SAVE_FORM => fp(&["defaultSaveForm"], Codec::PlainText),
            ss::F_DEFAULT_LOAD_FORM => fp(&["defaultLoadForm"], Codec::PlainText),
            ss::F_AUXILIARY_SAVE_FORM => fp(&["auxiliarySaveForm"], Codec::PlainText),
            ss::F_AUXILIARY_LOAD_FORM => fp(&["auxiliaryLoadForm"], Codec::PlainText),
            // objectBelonging/extendedConfigurationObject — structural-tail, не витнессится.
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let (tag, name_tag) = match collection {
            "Form" => ("forms", "name"),
            "Template" => ("templates", "name"),
            _ => return None,
        };
        Some(ChildLocus { container: &[], child_tag: tag, props_wrapped: false, name_tag, bare_ref: false })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        ss_bindings()
    }
}

// ===== FormRef (EDT — полный inline-стаб; usePurposes ПЕРЕМЕННЫЙ) =====
pub struct EdtFormRef;
impl LocusMap for EdtFormRef {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            fref::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            fref::F_COMMENT => fp(&["comment"], Codec::PlainText),
            fref::F_INCLUDE_HELP_IN_CONTENTS => fp(&["includeHelpInContents"], Codec::BoolPresence),
            fref::F_HELP => fp(&["help"], Codec::HelpConst),
            // ПЕРЕМЕННЫЙ список сиблингов `<usePurposes>X</usePurposes>` (1..2 witnessed;
            // `ВыборФинансовогоПериода` несёт только PersonalComputer) → RefList-Edt
            // (плоские текст-листы от корня стаба), НЕ UsePurposesConst.
            fref::F_USE_PURPOSES => fp(&["usePurposes"], Codec::RefList(RefListDialect::Edt)),
            _ => return None,
        })
    }
}

// ===== TemplateRef (EDT — inline-стаб; SSL: 0 макетов, reuse-proven форма) =====
pub struct EdtTemplateRef;
impl LocusMap for EdtTemplateRef {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            tref::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            tref::F_COMMENT => fp(&["comment"], Codec::PlainText),
            tref::F_TEMPLATE_TYPE => fp(&["templateType"], Codec::EnumText),
            _ => return None,
        })
    }
}

/// `&'static` бинды child-видов корня SettingsStorage.
fn ss_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Form", child_spec: settings_storage_form_ref(), child_map: &EdtFormRef },
            ChildBinding { collection: "Template", child_spec: settings_storage_template_ref(), child_map: &EdtTemplateRef },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("SettingsStorage", settings_storage(), &EdtSettingsStorage, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("SettingsStorage", settings_storage(), &EdtSettingsStorage, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/SettingsStorage. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "SettingsStorage",
    read,
    write,
    corpus_subpath: "coverage/edt/s6_misc/src/SettingsStorages",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
