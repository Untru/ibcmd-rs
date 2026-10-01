//! EDT-проекция вида `FunctionalOptionsParameter` (зеркало
//! `core/spec/metadata/functional_options_parameter.rs`, ARCHITECTURE.md §5). ТОЛЬКО
//! размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>/<value>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `use` → сиблинги `<use>Path</use>` под КОРНЕМ (`RefList(Edt)`; пустой → опущен).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::ref_list::RefListDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::functional_options_parameter::{F_COMMENT, F_SYNONYM, F_USE};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_USE: &[&str] = &["use"];

/// Карта проекции EDT для `FunctionalOptionsParameter`.
pub struct EdtFunctionalOptionsParameter;

impl EdtFunctionalOptionsParameter {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_USE {
            FieldProjection::new(flat(P_USE), Codec::RefList(RefListDialect::Edt))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtFunctionalOptionsParameter {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::functional_options_parameter::functional_options_parameter;

/// R-read для харнесса: `.mdo`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor(
        "FunctionalOptionsParameter",
        functional_options_parameter(),
        &EdtFunctionalOptionsParameter,
        bytes,
    )
    .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor(
        "FunctionalOptionsParameter",
        functional_options_parameter(),
        &EdtFunctionalOptionsParameter,
        obj,
    )
    .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/FunctionalOptionsParameter. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "FunctionalOptionsParameter",
    read,
    write,
    corpus_subpath: "coverage/edt/s8_linked/src/FunctionalOptionsParameters",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
