//! Designer-проекция вида `DefinedType` (зеркало
//! `core/spec/metadata/defined_type.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Это ИНОЙ синтаксис того же
//! спека, что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<DefinedType>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `type` → `<Type>` (`Type(TypeDialect::Designer)`): набор `<v8:Type>QName` + DENSE
//!   квалификаторы.
//!
//! Платформенный `<InternalInfo>` (сиблинг `<Properties>`, ДО неё) — НЕ в этой карте:
//! фреймится каркасом коннектора в `MetadataObject.internal_info`, не движком.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `DefinedType` — ВЫВОДИТСЯ из канонического спека
/// (`docs/APPROACH.md` §2.2): тег = `[Kind, "Properties", UpperCamel(имя)]`, кодек = по
/// value_kind. Список полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует.
pub struct DesignerDefinedType;

impl LocusMap for DesignerDefinedType {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, defined_type(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::defined_type::defined_type;

/// R-read для харнесса: `.xml`-байты DefinedType → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("DefinedType", defined_type(), &DesignerDefinedType, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR DefinedType → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("DefinedType", defined_type(), &DesignerDefinedType, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/DefinedType. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "DefinedType",
    read,
    write,
    corpus_subpath: "coverage/designer/s6_misc/DefinedTypes",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
