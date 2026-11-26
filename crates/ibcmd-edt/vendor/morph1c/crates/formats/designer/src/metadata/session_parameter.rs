//! Designer-проекция вида `SessionParameter` (зеркало
//! `core/spec/metadata/session_parameter.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Это ИНОЙ синтаксис того же
//! спека, что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<SessionParameter>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `type` → `<Type>` (`Type(TypeDialect::Designer)`): набор `<v8:Type>QName` + DENSE
//!   квалификаторы.
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`v8:Type`/квалификаторы) несут v8 — это разбирает кодек.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `SessionParameter` — ВЫВОДИТСЯ из канонического спека
/// (`docs/APPROACH.md` §2.2): тег = `[Kind, "Properties", UpperCamel(имя)]`, кодек = по
/// value_kind. Список полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует.
pub struct DesignerSessionParameter;

impl LocusMap for DesignerSessionParameter {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, session_parameter(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::session_parameter::session_parameter;

/// R-read для харнесса: `.xml`-байты SessionParameter → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("SessionParameter", session_parameter(), &DesignerSessionParameter, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR SessionParameter → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("SessionParameter", session_parameter(), &DesignerSessionParameter, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/SessionParameter. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "SessionParameter",
    read,
    write,
    corpus_subpath: "coverage/designer/s6_misc/SessionParameters",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
