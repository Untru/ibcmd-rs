//! Designer-проекция вида `CommonPicture` (зеркало `core/spec/metadata/common_picture.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек ТОНКОГО дескриптора — канонику
//! (id/порядок/дефолты/нормализацию) держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! # Раскладка Designer picture (ТРИ файла)
//! * главный `<Name>.xml` — тонкий дескриптор (эта карта): DENSE (эмитятся и дефолты —
//!   `<Comment/>`, `<AvailabilityForChoice>false</…>`).
//! * `<Name>/Ext/Picture.xml` — константный ExtPicture-дескриптор (byte-exact реконструкция
//!   из имени бинаря, `formats_xml::picture_sidecar`; testkit BinarySidecar).
//! * `<Name>/Ext/Picture/Picture.<ext>` — БИНАРЬ картинки (Blob, §1.0 as-is; testkit).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от корня `<MetaDataObject>` через
//! `<CommonPicture>/<Properties>` (UpperCamelCase, property-теги без префикса, ns="").

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `CommonPicture` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
///
/// `transparentPixel` (List) деривация НЕ проецирует — НАМЕРЕННО: Designer-дескриптор
/// узла не несёт (witnessed ERP `ВажностиНовостей.xml` — только 4 тонких свойства);
/// носитель — обёртка сайдкара `Ext/Picture.xml` (`<xr:LoadTransparent>true</…>` +
/// `<xr:TransparentPixel x=… y=…/>`), её читает/пишет `morph1c_pipeline::picture_read`
/// В ТО ЖЕ спек-поле (зеркало `predefined`-паттерна; в спеке поле `x_ignore`).
pub struct DesignerCommonPicture;

impl LocusMap for DesignerCommonPicture {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, common_picture(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_picture::common_picture;

/// R-read для харнесса: `.xml`-байты CommonPicture → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonPicture", common_picture(), &DesignerCommonPicture, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonPicture → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonPicture", common_picture(), &DesignerCommonPicture, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommonPicture. Имя `HARNESS_ENTRY` — контракт `build.rs`.
///
/// [`CorpusLayout::BinarySidecar`]: дескриптор `<Name>.xml` (FilePerObject), бинарный спутник —
/// `<Name>/Ext/Picture/Picture.<ext>` (`sidecar_subdir="Ext/Picture"`), вспом. XML-дескриптор —
/// `<Name>/Ext/Picture.xml` (`sidecar_xml_subdir="Ext"`). Testkit округляет все byte-exact.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommonPicture",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommonPictures",
    layout: formats_xml::CorpusLayout::BinarySidecar {
        descriptor_ext: "xml",
        descriptor_dir_per_object: false,
        sidecar_subdir: "Ext/Picture",
        sidecar_stem: "Picture",
        sidecar_xml_subdir: "Ext",
    },
};
