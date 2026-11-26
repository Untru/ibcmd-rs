//! EDT-проекция вида `SessionParameter` (зеркало `core/spec/metadata/session_parameter.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>lang</key><value>text</value>`
//!   (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `type` → `<type>` (`Type(TypeDialect::Edt)`): набор `<types>id</types>` + sparse
//!   квалификаторы.
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции EDT для `SessionParameter` — ВЫВОДИТСЯ из канонического спека
/// (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации.
pub struct EdtSessionParameter;

impl LocusMap for EdtSessionParameter {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, session_parameter(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::session_parameter::session_parameter;

/// R-read для харнесса: `.mdo`-байты SessionParameter → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("SessionParameter", session_parameter(), &EdtSessionParameter, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR SessionParameter → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("SessionParameter", session_parameter(), &EdtSessionParameter, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/SessionParameter. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "SessionParameter",
    read,
    write,
    corpus_subpath: "coverage/edt/s6_misc/src/SessionParameters",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
