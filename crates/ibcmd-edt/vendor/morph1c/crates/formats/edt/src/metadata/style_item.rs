//! EDT-проекция вида `StyleItem` (зеркало `core/spec/metadata/style_item.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)` — плоские дети корня `<mdclass:StyleItem>`:
//! * `synonym` → `LocalizedKeyVal`;
//! * `comment` → `PlainText` (дефолт `""` опущен);
//! * `type` → `EnumText` (дефолт `Color` опущен; `<type>Font</type>` эмитится);
//! * `value` → `StyleValue(Edt)` — двухуровневый `<value xsi:type="core:FontValue">
//!   <value xsi:type="core:FontRef|FontDef">…` (mirror Color).
//!
//! `objectBelonging`/`extendedConfigurationObject` в корпусе не встречены → не маппятся.
//! Корень несёт `xmlns:xsi`+`xmlns:core` (нужны стиль-кодеку: `xsi:type="core:FontValue"`).

use formats_xml::{Codec, FieldProjection, LocusMap, StyleValueDialect, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::style_item::{F_COMMENT, F_SYNONYM, F_TYPE, F_VALUE};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_TYPE: &[&str] = &["type"];
const P_VALUE: &[&str] = &["value"];

/// Карта проекции EDT для `StyleItem`.
pub struct EdtStyleItem;

impl EdtStyleItem {
    /// Строка проекции для поля (или `None`, если поле не из этого вида/не проецируется).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_TYPE {
            FieldProjection::new(flat(P_TYPE), Codec::EnumText)
        } else if field == F_VALUE {
            FieldProjection::new(flat(P_VALUE), Codec::StyleValue(StyleValueDialect::Edt))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtStyleItem {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // EDT StyleItem-корень несёт xmlns:xsi+xmlns:core (нужны стиль-кодеку:
        // `<value xsi:type="core:FontValue">`). Порядок = эталона (xsi, затем core; оба
        // ДО xmlns:mdclass).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::style_item::style_item;

/// R-read для харнесса: `.mdo`-байты StyleItem → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("StyleItem", style_item(), &EdtStyleItem, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR StyleItem → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("StyleItem", style_item(), &EdtStyleItem, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/StyleItem. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "StyleItem",
    read,
    write,
    corpus_subpath: "coverage/edt/s6_misc/src/StyleItems",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
