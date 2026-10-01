//! EDT-проекция вида `CommandGroup` (зеркало `core/spec/metadata/command_group.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>lang</key><value>text</value>`
//!   (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `representation` → `<representation>литерал</representation>` (`EnumText`);
//! * `toolTip` → контейнер `<toolTip>` с парами `<key>/<value>` (`LocalizedKeyVal`; дефолт опущен);
//! * `picture` → `<picture xsi:type="core:PictureRef"><picture>ref</picture></picture>`
//!   (`PictureRef`; дефолт пустой опущен);
//! * `category` → `<category>литерал</category>` (`EnumText`).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::command_group::{
    F_CATEGORY, F_COMMENT, F_PICTURE, F_REPRESENTATION, F_SYNONYM, F_TOOLTIP,
};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_REPRESENTATION: &[&str] = &["representation"];
const P_TOOLTIP: &[&str] = &["toolTip"];
const P_PICTURE: &[&str] = &["picture"];
const P_CATEGORY: &[&str] = &["category"];

/// Карта проекции EDT для `CommandGroup`.
pub struct EdtCommandGroup;

impl EdtCommandGroup {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_REPRESENTATION {
            FieldProjection::new(flat(P_REPRESENTATION), Codec::EnumText)
        } else if field == F_TOOLTIP {
            FieldProjection::new(flat(P_TOOLTIP), Codec::LocalizedKeyVal)
        } else if field == F_PICTURE {
            FieldProjection::new(flat(P_PICTURE), Codec::PictureRef(formats_xml::picture::PictureDialect::Edt))
        } else if field == F_CATEGORY {
            FieldProjection::new(flat(P_CATEGORY), Codec::EnumText)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtCommandGroup {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }

    fn root_extra_namespaces_optional(&self) -> bool {
        // xsi+core объявляются на корне группы команд ЛИШЬ при непустом <picture>
        // (его `xsi:type="core:PictureRef"` требует обоих ns). Сверено по корпусу SSL:
        // 5/6 объявляют ns ⟺ 5/6 несут непустую картинку; «Информация» (без картинки) — нет.
        true
    }

    fn root_extra_namespaces_present(&self, obj: &morph1c_core::ir::MetadataObject) -> bool {
        // ns нужны ⟺ picture-поле непусто (`Str` ≠ "").
        obj.properties.iter().any(|(id, v)| {
            *id == F_PICTURE
                && matches!(v, morph1c_core::ir::value::PropertyValue::Str(s) if !s.is_empty())
        })
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::command_group::command_group;

/// R-read для харнесса: `.mdo`-байты CommandGroup → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommandGroup", command_group(), &EdtCommandGroup, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommandGroup → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommandGroup", command_group(), &EdtCommandGroup, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/CommandGroup. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CommandGroup",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/CommandGroups",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
