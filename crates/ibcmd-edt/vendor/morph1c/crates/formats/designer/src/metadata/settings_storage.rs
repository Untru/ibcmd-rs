//! Designer-проекция вида `SettingsStorage` + его дочерних видов (зеркало
//! `core/spec/metadata/settings_storage*.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5). Платформенный `<InternalInfo>`
//! (одна категория Manager) фреймится каркасом коннектора (`PRODUCED_CATEGORIES`).
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<SettingsStorage>/<Properties>`, DENSE
//! (6 полей: Synonym, Comment, DefaultSaveForm, DefaultLoadForm, AuxiliarySaveForm,
//! AuxiliaryLoadForm — сверено 1/1 SSL). Forms/Templates — BARE-ссылки `<Form>Имя</Form>`
//! (тело формы — отдельный файл; стаб-метаданные несёт только EDT).

use formats_xml::children::ChildBinding;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;

use morph1c_core::spec::metadata::settings_storage as ss;
use morph1c_core::spec::metadata::settings_storage::settings_storage;
use morph1c_core::spec::metadata::settings_storage_form_ref::settings_storage_form_ref;
use morph1c_core::spec::metadata::settings_storage_template_ref::settings_storage_template_ref;

const fn elem(path: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path, ns: "" }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path), codec }
}

// Пути от корня <MetaDataObject> через <SettingsStorage>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["SettingsStorage", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerSettingsStorage;

impl LocusMap for DesignerSettingsStorage {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            ss::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            ss::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            ss::F_DEFAULT_SAVE_FORM => fp(p!("DefaultSaveForm"), Codec::PlainText),
            ss::F_DEFAULT_LOAD_FORM => fp(p!("DefaultLoadForm"), Codec::PlainText),
            ss::F_AUXILIARY_SAVE_FORM => fp(p!("AuxiliarySaveForm"), Codec::PlainText),
            ss::F_AUXILIARY_LOAD_FORM => fp(p!("AuxiliaryLoadForm"), Codec::PlainText),
            // objectBelonging/extendedConfigurationObject — structural-tail, не витнессится.
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["SettingsStorage", "ChildObjects"];
        let tag = match collection {
            "Form" => "Form",
            "Template" => "Template",
            _ => return None,
        };
        Some(ChildLocus { container: cont, child_tag: tag, props_wrapped: false, name_tag: "Name", bare_ref: true })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        ss_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Единственный SSL-объект несёт `<ChildObjects>` (13 форм); зеркалит сиблингов
        // (Report/DataProcessor: контейнер эмитится всегда). Пустой SettingsStorage в
        // корпусе не witnessed — путь валидируется R-гейтом при появлении.
        true
    }
}

// FormRef/TemplateRef — bare-ссылки (LocusMap не проецирует свойств).
pub struct DesignerFormRef;
impl LocusMap for DesignerFormRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}

pub struct DesignerTemplateRef;
impl LocusMap for DesignerTemplateRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}

fn ss_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![
            ChildBinding { collection: "Form", child_spec: settings_storage_form_ref(), child_map: &DesignerFormRef },
            ChildBinding { collection: "Template", child_spec: settings_storage_template_ref(), child_map: &DesignerTemplateRef },
        ]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("SettingsStorage", settings_storage(), &DesignerSettingsStorage, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("SettingsStorage", settings_storage(), &DesignerSettingsStorage, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/SettingsStorage. Дети покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "SettingsStorage",
    read,
    write,
    corpus_subpath: "coverage/designer/s6_misc/SettingsStorages",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
