//! Designer-проекция вида `WSReference` (зеркало `core/spec/metadata/w_s_reference.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Это ИНОЙ синтаксис того же спека, что и EDT, поэтому оба формата дают РАВНЫЙ
//! IR (§3.5). Designer DENSE — все поля present всегда (дефолты эмитятся: `<Comment/>`).
//! Порядок = спек-порядок (DENSE), `field_emit_order` не нужен. Платформенный
//! `<InternalInfo>` (единственный `Manager`) фреймится каркасом коннектора
//! (`formats_xml::produced_types` через реестр `PRODUCED_CATEGORIES`), не тут.
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<WSReference>/<Properties>`:
//! * `synonym` → `<Synonym>` (`LocalizedV8`);
//! * `comment` → `<Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `locationURL` → `<LocationURL>` (`PlainText`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `WsReference` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerWsReference;

impl LocusMap for DesignerWsReference {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, w_s_reference(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::w_s_reference::w_s_reference;

/// R-read для харнесса: `.xml`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("WSReference", w_s_reference(), &DesignerWsReference, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("WSReference", w_s_reference(), &DesignerWsReference, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/WSReference. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "WSReference",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/WSReferences",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
