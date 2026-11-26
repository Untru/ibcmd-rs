//! Designer-проекция вида `CommonModule` (зеркало `core/spec/metadata/common_module.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику (id/kind/порядок/
//! дефолты/нормализацию) держит `core/spec` (§1.6). Это ИНОЙ синтаксис того же спека,
//! что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>`
//! (его коннектор передаёт движку как `source.root` целиком, чтобы тотальность §1.0
//! считалась по ВСЕМУ дереву, а не только по `<Properties>`-поддереву). Каждый путь —
//! `["CommonModule","Properties","<Tag>"]`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`, ns v8);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * 7 bool-флагов → `<Flag>true|false</Flag>` (`BoolText`, текст-литерал);
//! * `returnValuesReuse` → `<ReturnValuesReuse>Literal</ReturnValuesReuse>` (`EnumText`).
//!
//! Имена тегов — UpperCamelCase (Designer-конвенция), property-теги БЕЗ префикса
//! (дефолтный MDClasses-ns, `ns=""`); только синоним-инеры несут префикс `v8`.
//! Порядок детей в файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — имена.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `CommonModule` — ВЫВОДИТСЯ из канонического спека
/// (`docs/APPROACH.md` §2.2): тег = `[Kind, "Properties", UpperCamel(имя)]`, кодек = по
/// value_kind. Список полей/порядок/дефолты держит `core/spec` (§1.6); эта карта лишь
/// делегирует деривации (`formats_xml::derive`) — заменяет рукописную таблицу.
pub struct DesignerCommonModule;

impl LocusMap for DesignerCommonModule {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, common_module(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---
//
// Type-erased обёртки (ошибка → строкой) над kind-параметризованным путём коннектора.
// `HARNESS_ENTRY` локальна ЭТОМУ файлу — добавление вида не правит общих файлов (§5).
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_module::common_module;

/// R-read для харнесса: `.xml`-байты CommonModule → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonModule", common_module(), &DesignerCommonModule, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonModule → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonModule", common_module(), &DesignerCommonModule, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommonModule. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommonModule",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommonModules",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
