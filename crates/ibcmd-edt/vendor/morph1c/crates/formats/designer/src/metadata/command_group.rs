//! Designer-проекция вида `CommandGroup` (зеркало `core/spec/metadata/command_group.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Это ИНОЙ синтаксис того же спека, что и EDT, поэтому оба формата дают РАВНЫЙ
//! IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<CommandGroup>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `representation` → `<Representation>литерал</Representation>` (`EnumText`);
//! * `toolTip` → `<ToolTip>` с `v8:item/...` (`LocalizedV8`; пустой → `<ToolTip/>`);
//! * `picture` → `<Picture><xr:Ref>ref</xr:Ref><xr:LoadTransparent>…</…></Picture>`
//!   (`PictureRef`; пустой → `<Picture/>`);
//! * `category` → `<Category>литерал</Category>` (`EnumText`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`xr:Ref`) несут v8/xr — это разбирает кодек.

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::command_group::{
    F_CATEGORY, F_COMMENT, F_PICTURE, F_REPRESENTATION, F_SYNONYM, F_TOOLTIP,
};

/// `PropElement{path, ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["CommandGroup", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["CommandGroup", "Properties", "Comment"];
const P_REPRESENTATION: &[&str] = &["CommandGroup", "Properties", "Representation"];
const P_TOOLTIP: &[&str] = &["CommandGroup", "Properties", "ToolTip"];
const P_PICTURE: &[&str] = &["CommandGroup", "Properties", "Picture"];
const P_CATEGORY: &[&str] = &["CommandGroup", "Properties", "Category"];

/// Карта проекции Designer для `CommandGroup`.
pub struct DesignerCommandGroup;

impl DesignerCommandGroup {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_REPRESENTATION {
            FieldProjection::new(elem(P_REPRESENTATION, ""), Codec::EnumText)
        } else if field == F_TOOLTIP {
            FieldProjection::new(elem(P_TOOLTIP, ""), Codec::LocalizedV8)
        } else if field == F_PICTURE {
            FieldProjection::new(elem(P_PICTURE, ""), Codec::PictureRef(formats_xml::picture::PictureDialect::Designer))
        } else if field == F_CATEGORY {
            FieldProjection::new(elem(P_CATEGORY, ""), Codec::EnumText)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerCommandGroup {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::command_group::command_group;

/// R-read для харнесса: `.xml`-байты CommandGroup → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommandGroup", command_group(), &DesignerCommandGroup, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommandGroup → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommandGroup", command_group(), &DesignerCommandGroup, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommandGroup. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommandGroup",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommandGroups",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
