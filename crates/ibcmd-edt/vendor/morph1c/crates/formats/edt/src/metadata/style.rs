//! EDT-проекция вида `Style` (зеркало `core/spec/metadata/style.rs`, ARCHITECTURE.md §5).
//! ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)` — плоские дети корня `<mdclass:Style>`:
//! * `synonym` → `LocalizedKeyVal`;
//! * `comment` → `PlainText` (дефолт `""` опущен).
//!
//! `objectBelonging`/`extendedConfigurationObject` в корпусе не встречены → не маппятся.
//! Корень несёт ТОЛЬКО `xmlns:mdclass` (в отличие от StyleItem — нет стиль-значения, а
//! значит нет `xmlns:xsi`/`xmlns:core`) → `root_extra_namespaces` дефолтный (пусто).

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::style::{F_COMMENT, F_SYNONYM};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];

/// Карта проекции EDT для `Style`.
pub struct EdtStyle;

impl EdtStyle {
    /// Строка проекции для поля (или `None`, если поле не из этого вида/не проецируется).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtStyle {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::style::style;

/// R-read для харнесса: `.mdo`-байты Style → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Style", style(), &EdtStyle, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Style → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Style", style(), &EdtStyle, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Style. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Style",
    read,
    write,
    corpus_subpath: "coverage/edt/s6_misc/src/Styles",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
