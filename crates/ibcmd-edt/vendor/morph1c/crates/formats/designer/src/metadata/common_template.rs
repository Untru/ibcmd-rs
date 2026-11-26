//! Designer-проекция вида `CommonTemplate` (зеркало
//! `core/spec/metadata/common_template.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику (id/kind/порядок/дефолты/нормализацию) держит `core/spec`
//! (§1.6). Это ИНОЙ синтаксис того же спека, что и EDT, поэтому оба формата дают РАВНЫЙ
//! IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>`
//! (его коннектор передаёт движку как `source.root` целиком, чтобы тотальность §1.0
//! считалась по ВСЕМУ дереву). Каждый путь — `["CommonTemplate","Properties","<Tag>"]`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`, ns v8);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `templateType` → `<TemplateType>Literal</TemplateType>` (`EnumText`; DENSE —
//!   эмитится всегда, включая дефолт `SpreadsheetDocument`).
//!
//! Имена тегов — UpperCamelCase (Designer-конвенция), property-теги БЕЗ префикса
//! (дефолтный MDClasses-ns, `ns=""`); только синоним-инеры несут префикс `v8`.
//! Порядок детей в файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — имена.
//! Тело макета (`Ext/Template.*`) — ВНЕ дескриптора (отдельный Blob-артефакт).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `CommonTemplate` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerCommonTemplate;

impl LocusMap for DesignerCommonTemplate {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, common_template(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---
//
// Type-erased обёртки (ошибка → строкой) над kind-параметризованным путём коннектора.
// `HARNESS_ENTRY` локальна ЭТОМУ файлу — добавление вида не правит общих файлов (§5).
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_template::common_template;

/// R-read для харнесса: `.xml`-байты CommonTemplate → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonTemplate", common_template(), &DesignerCommonTemplate, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonTemplate → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonTemplate", common_template(), &DesignerCommonTemplate, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommonTemplate. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommonTemplate",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommonTemplates",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
