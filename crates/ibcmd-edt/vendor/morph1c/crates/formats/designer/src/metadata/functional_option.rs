//! Designer-проекция вида `FunctionalOption` (зеркало
//! `core/spec/metadata/functional_option.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Это ИНОЙ синтаксис того же
//! спека, что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<FunctionalOption>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `location` → `<Location>Path</Location>` (`PlainText`);
//! * `privilegedGetMode` → `<PrivilegedGetMode>true|false</PrivilegedGetMode>` (`BoolText`);
//! * `content` → контейнер `<Content>` с `<xr:Object>Path</xr:Object>`
//!   (`RefList(DesignerObject)`; пустой → `<Content/>`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`xr:Object`) несут v8/xr — это разбирает кодек.

use formats_xml::ref_list::RefListDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::functional_option::{
    F_COMMENT, F_CONTENT, F_LOCATION, F_PRIVILEGED_GET_MODE, F_SYNONYM,
};

/// `PropElement{path, ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["FunctionalOption", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["FunctionalOption", "Properties", "Comment"];
const P_LOCATION: &[&str] = &["FunctionalOption", "Properties", "Location"];
const P_PRIVILEGED: &[&str] = &["FunctionalOption", "Properties", "PrivilegedGetMode"];
const P_CONTENT: &[&str] = &["FunctionalOption", "Properties", "Content"];

/// Карта проекции Designer для `FunctionalOption`.
pub struct DesignerFunctionalOption;

impl DesignerFunctionalOption {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_LOCATION {
            FieldProjection::new(elem(P_LOCATION, ""), Codec::PlainText)
        } else if field == F_PRIVILEGED_GET_MODE {
            FieldProjection::new(elem(P_PRIVILEGED, ""), Codec::BoolText)
        } else if field == F_CONTENT {
            FieldProjection::new(elem(P_CONTENT, ""), Codec::RefList(RefListDialect::DesignerObject))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerFunctionalOption {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::functional_option::functional_option;

/// R-read для харнесса: `.xml`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("FunctionalOption", functional_option(), &DesignerFunctionalOption, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("FunctionalOption", functional_option(), &DesignerFunctionalOption, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/FunctionalOption. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "FunctionalOption",
    read,
    write,
    corpus_subpath: "coverage/designer/s8_linked/FunctionalOptions",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
