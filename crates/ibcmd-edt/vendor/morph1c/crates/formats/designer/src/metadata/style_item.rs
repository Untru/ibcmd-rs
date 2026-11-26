//! Designer-проекция вида `StyleItem` (зеркало `core/spec/metadata/style_item.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от корня `<MetaDataObject>` через
//! `<StyleItem>/<Properties>`, DENSE (даже дефолты эмитятся):
//! * `synonym` → `LocalizedV8`;
//! * `comment` → `PlainText` (пустой → `<Comment/>`);
//! * `type` → `EnumText` (DENSE: `<Type>Color</Type>` эмитится даже при дефолте);
//! * `value` → `StyleValue(Designer)` — плоский `<Value xsi:type="v8ui:Font" …/>` /
//!   `<Value xsi:type="v8ui:Color">#RRGGBB|pref:Name</Value>`.
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/xsi-атрибуты) несут v8/xsi — их разбирают кодеки.

use formats_xml::{Codec, FieldProjection, LocusMap, StyleValueDialect, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::style_item::{F_COMMENT, F_SYNONYM, F_TYPE, F_VALUE};

/// `PropElement{path:[tags], ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["StyleItem", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["StyleItem", "Properties", "Comment"];
const P_TYPE: &[&str] = &["StyleItem", "Properties", "Type"];
const P_VALUE: &[&str] = &["StyleItem", "Properties", "Value"];

/// Карта проекции Designer для `StyleItem`.
pub struct DesignerStyleItem;

impl DesignerStyleItem {
    /// Строка проекции для поля (или `None`, если поле не из этого вида/не проецируется).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_TYPE {
            FieldProjection::new(elem(P_TYPE, ""), Codec::EnumText)
        } else if field == F_VALUE {
            FieldProjection::new(elem(P_VALUE, ""), Codec::StyleValue(StyleValueDialect::Designer))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerStyleItem {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::style_item::style_item;

/// R-read для харнесса: `.xml`-байты StyleItem → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("StyleItem", style_item(), &DesignerStyleItem, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR StyleItem → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("StyleItem", style_item(), &DesignerStyleItem, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/StyleItem. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "StyleItem",
    read,
    write,
    corpus_subpath: "coverage/designer/s6_misc/StyleItems",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
