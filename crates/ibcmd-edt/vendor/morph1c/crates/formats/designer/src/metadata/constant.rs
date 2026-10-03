//! Designer-проекция вида `Constant` (зеркало `core/spec/metadata/constant.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)` ВЫВОДИТСЯ из канонического спека
//! (`docs/APPROACH.md` §2.2): пути от корня `<MetaDataObject>` через
//! `<Constant>/<Properties>/<UpperCamel(имя)>`, кодек по value_kind
//! (`localized→LocalizedV8`, `bool→BoolText`, `enum→EnumText`, `str→PlainText`,
//! `type→Type(Designer)`, `value→Value(Designer)`). DENSE-эмиссия (даже дефолты) —
//! решает движок+спек, не карта.
//!
//! Платформенный `<InternalInfo>` (Manager/ValueManager/ValueKey) — каркас коннектора
//! (§3.5), не эта карта. Property-теги — UpperCamelCase БЕЗ префикса (`ns=""`);
//! внутренности (`v8:item`/`v8:Type`/xsi-атрибуты) несут v8/xsi — это разбирают кодеки.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::constant as c;

/// Карта проекции Designer для `Constant` — ВЫВОДИТСЯ из спека (`formats_xml::derive`),
/// заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerConstant;

impl LocusMap for DesignerConstant {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        // Структурные List-поля не деривятся по value_kind (§2.2) — ручные ветки
        // (ERP-witnessed на корне Constant; designer-пути через <Constant>/<Properties>).
        match field {
            c::F_CHOICE_PARAMETER_LINKS => Some(FieldProjection {
                locus: XmlLocus::PropElement {
                    path: &["Constant", "Properties", "ChoiceParameterLinks"],
                    ns: "",
                },
                codec: Codec::ChoiceParameterLinks(
                    formats_xml::choice_param_links::LinksDialect::Designer,
                ),
            }),
            c::F_CHOICE_PARAMETERS => Some(FieldProjection {
                locus: XmlLocus::PropElement {
                    path: &["Constant", "Properties", "ChoiceParameters"],
                    ns: "",
                },
                codec: Codec::ChoiceParameters(
                    formats_xml::choice_parameters::ChoiceParametersDialect::Designer,
                ),
            }),
            _ => projection(DeriveDialect::Designer, constant(), field),
        }
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::constant::constant;

/// R-read для харнесса: `.xml`-байты Constant → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Constant", constant(), &DesignerConstant, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Constant → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Constant", constant(), &DesignerConstant, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Constant. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Constant",
    read,
    write,
    corpus_subpath: "coverage/designer/s1_core/Constants",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
