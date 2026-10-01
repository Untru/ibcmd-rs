//! EDT-проекция вида `DefinedType` (зеркало `core/spec/metadata/defined_type.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)` (плоские дети корня, как у SessionParameter):
//! * `synonym` → контейнер `<synonym>` с парами `<key>lang</key><value>text</value>`
//!   (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `type` → `<type>` (`Type(TypeDialect::Edt)`): набор `<types>id</types>` + sparse
//!   квалификаторы.
//!
//! Платформенный `<producedTypes>` — НЕ в этой карте: он лежит структурно ДО `<name>`,
//! поэтому фреймится каркасом коннектора в `MetadataObject.internal_info`, а не движком.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции EDT для `DefinedType` — ВЫВОДИТСЯ из канонического спека
/// (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации.
pub struct EdtDefinedType;

impl LocusMap for EdtDefinedType {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, defined_type(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::defined_type::defined_type;

/// R-read для харнесса: `.mdo`-байты DefinedType → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("DefinedType", defined_type(), &EdtDefinedType, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR DefinedType → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("DefinedType", defined_type(), &EdtDefinedType, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/DefinedType. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "DefinedType",
    read,
    write,
    corpus_subpath: "coverage/edt/s6_misc/src/DefinedTypes",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
