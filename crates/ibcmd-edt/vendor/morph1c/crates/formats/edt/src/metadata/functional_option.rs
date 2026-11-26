//! EDT-проекция вида `FunctionalOption` (зеркало `core/spec/metadata/functional_option.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>/<value>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `location` → `<location>Path</location>` (`PlainText`);
//! * `privilegedGetMode` → `<privilegedGetMode>true</privilegedGetMode>` (`BoolPresence`;
//!   дефолт false → опущен);
//! * `content` → сиблинги `<content>Path</content>` под КОРНЕМ (`RefList(Edt)`; пустой → опущен).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::ref_list::RefListDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::functional_option::{
    F_COMMENT, F_CONTENT, F_LOCATION, F_PRIVILEGED_GET_MODE, F_SYNONYM,
};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_LOCATION: &[&str] = &["location"];
const P_PRIVILEGED: &[&str] = &["privilegedGetMode"];
const P_CONTENT: &[&str] = &["content"];

/// Карта проекции EDT для `FunctionalOption`.
pub struct EdtFunctionalOption;

impl EdtFunctionalOption {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_LOCATION {
            FieldProjection::new(flat(P_LOCATION), Codec::PlainText)
        } else if field == F_PRIVILEGED_GET_MODE {
            FieldProjection::new(flat(P_PRIVILEGED), Codec::BoolPresence)
        } else if field == F_CONTENT {
            FieldProjection::new(flat(P_CONTENT), Codec::RefList(RefListDialect::Edt))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtFunctionalOption {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::functional_option::functional_option;

/// R-read для харнесса: `.mdo`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("FunctionalOption", functional_option(), &EdtFunctionalOption, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("FunctionalOption", functional_option(), &EdtFunctionalOption, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/FunctionalOption. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "FunctionalOption",
    read,
    write,
    corpus_subpath: "coverage/edt/s8_linked/src/FunctionalOptions",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
