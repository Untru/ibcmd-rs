//! Designer-проекция вида `FunctionalOptionsParameter` (зеркало
//! `core/spec/metadata/functional_options_parameter.rs`, ARCHITECTURE.md §5). ТОЛЬКО
//! размещение/кодировка ячеек — канонику держит `core/spec` (§1.6). Это ИНОЙ синтаксис
//! того же спека, что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<FunctionalOptionsParameter>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `use` → контейнер `<Use>` с `<xr:Item xsi:type="xr:MDObjectRef">Path</xr:Item>`
//!   (`RefList(DesignerItem)`; пустой → `<Use/>`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`xr:Item`) несут v8/xr — это разбирает кодек.

use formats_xml::ref_list::RefListDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::functional_options_parameter::{F_COMMENT, F_SYNONYM, F_USE};

/// `PropElement{path, ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["FunctionalOptionsParameter", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["FunctionalOptionsParameter", "Properties", "Comment"];
const P_USE: &[&str] = &["FunctionalOptionsParameter", "Properties", "Use"];

/// Карта проекции Designer для `FunctionalOptionsParameter`.
pub struct DesignerFunctionalOptionsParameter;

impl DesignerFunctionalOptionsParameter {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_USE {
            FieldProjection::new(elem(P_USE, ""), Codec::RefList(RefListDialect::DesignerItem))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerFunctionalOptionsParameter {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::functional_options_parameter::functional_options_parameter;

/// R-read для харнесса: `.xml`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor(
        "FunctionalOptionsParameter",
        functional_options_parameter(),
        &DesignerFunctionalOptionsParameter,
        bytes,
    )
    .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor(
        "FunctionalOptionsParameter",
        functional_options_parameter(),
        &DesignerFunctionalOptionsParameter,
        obj,
    )
    .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/FunctionalOptionsParameter. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "FunctionalOptionsParameter",
    read,
    write,
    corpus_subpath: "coverage/designer/s8_linked/FunctionalOptionsParameters",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
