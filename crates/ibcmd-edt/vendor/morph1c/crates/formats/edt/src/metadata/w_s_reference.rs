//! EDT-проекция вида `WSReference` (зеркало `core/spec/metadata/w_s_reference.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Платформенный `<producedTypes>` (единственный `Manager`) фреймится каркасом
//! коннектора (`formats_xml::produced_types` через реестр `PRODUCED_CATEGORIES`), не тут.
//!
//! Карта `FieldId → (XmlLocus, Codec)` (EDT SPARSE — дефолты опущены):
//! * `synonym` → `<synonym>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>` (`PlainText`; дефолт "" опущен);
//! * `locationURL` → `<locationURL>` (`PlainText`; дефолт "" опущен).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). EDT-порядок
//! present-полей совпадает с каноническим спек-порядком (`comment` дефолтен и опущен),
//! поэтому `field_emit_order` не нужен. Корень несёт лишь `mdclass`-ns (envelope) —
//! дополнительных namespace'ов у листа нет.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Edt для `WsReference` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtWsReference;

impl LocusMap for EdtWsReference {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, w_s_reference(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::w_s_reference::w_s_reference;

/// R-read для харнесса: `.mdo`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("WSReference", w_s_reference(), &EdtWsReference, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("WSReference", w_s_reference(), &EdtWsReference, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/WSReference. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "WSReference",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/WSReferences",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
