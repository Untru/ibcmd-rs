//! Designer-проекция вида `Style` (зеркало `core/spec/metadata/style.rs`, ARCHITECTURE.md
//! §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec` (§1.6). Иной
//! синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от корня `<MetaDataObject>` через
//! `<Style>/<Properties>`:
//! * `synonym` → `LocalizedV8`;
//! * `comment` → `PlainText`.
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`) несут v8 — их разбирает кодек.

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::style::{F_COMMENT, F_SYNONYM};

/// `PropElement{path:[tags], ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["Style", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["Style", "Properties", "Comment"];

/// Карта проекции Designer для `Style`.
pub struct DesignerStyle;

impl DesignerStyle {
    /// Строка проекции для поля (или `None`, если поле не из этого вида/не проецируется).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerStyle {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::style::style;

/// R-read для харнесса: `.xml`-байты Style → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Style", style(), &DesignerStyle, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Style → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Style", style(), &DesignerStyle, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Style. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Style",
    read,
    write,
    corpus_subpath: "coverage/designer/s6_misc/Styles",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
