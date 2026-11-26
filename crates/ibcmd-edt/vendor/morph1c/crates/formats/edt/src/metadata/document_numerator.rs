//! EDT-проекция вида `DocumentNumerator` (зеркало
//! `core/spec/metadata/document_numerator.rs`, ARCHITECTURE.md §5). ТОЛЬКО
//! размещение/кодировка ячеек — канонику держит `core/spec` (§1.6).
//!
//! Карта `FieldId → (XmlLocus, Codec)` (EDT SPARSE — дефолты опущены):
//! * `synonym` → `<synonym>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>` (`PlainText`; дефолт "" опущен);
//! * `numberType` → `<numberType>` (`EnumText`; дефолт Number опущен);
//! * `numberLength` → `<numberLength>N</…>` (`IntText`; дефолт 0 опущен);
//! * `numberAllowedLength` → `<numberAllowedLength>` (`EnumText`; дефолт Fixed опущен);
//! * `numberPeriodicity` → `<numberPeriodicity>` (`EnumText`; дефолт Nonperiodical опущен);
//! * `checkUnique` → `<checkUnique>true</checkUnique>` (`BoolPresence`; дефолт false опущен).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). EDT-порядок
//! present-полей совпадает с каноническим спек-порядком (`comment` всегда дефолтен и
//! опущен), поэтому `field_emit_order` не нужен. Корень несёт лишь `mdclass`-ns
//! (envelope) — дополнительных namespace'ов у листа нет.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Edt для `DocumentNumerator` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtDocumentNumerator;

impl LocusMap for EdtDocumentNumerator {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, document_numerator(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::document_numerator::document_numerator;

/// R-read для харнесса: `.mdo`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("DocumentNumerator", document_numerator(), &EdtDocumentNumerator, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("DocumentNumerator", document_numerator(), &EdtDocumentNumerator, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/DocumentNumerator. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "DocumentNumerator",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/DocumentNumerators",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
