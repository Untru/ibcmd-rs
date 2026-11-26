//! Designer-проекция вида `XDTOPackage` (зеркало `core/spec/metadata/x_d_t_o_package.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек ТОНКОГО дескриптора — канонику
//! (id/порядок/дефолты/нормализацию) держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! # Раскладка Designer XDTOPackage (главный дескриптор + текстовый спутник схемы)
//! * главный `<Name>.xml` — тонкий дескриптор (эта карта): DENSE (эмитятся и дефолты —
//!   `<Comment/>`).
//! * `<Name>/Ext/Package.bin` — текстовый спутник XDTO-схемы (ns `…/8.1/xdto`, root
//!   `<package>`); round-trip byte-exact через XDTO-schema-кодек (`formats_xml::xdto` c
//!   `SidecarFormat::DesignerXdto`; testkit TextSidecar). Тело идентично EDT-`Package.xdto`
//!   (различие лишь BOM), поэтому cross-format X по дереву схемы.
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от корня `<MetaDataObject>` через
//! `<XDTOPackage>/<Properties>` (UpperCamelCase, property-теги без префикса, ns="").

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `XdtoPackage` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerXdtoPackage;

impl LocusMap for DesignerXdtoPackage {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, x_d_t_o_package(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::x_d_t_o_package::x_d_t_o_package;

/// R-read для харнесса: `.xml`-байты XDTOPackage → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("XDTOPackage", x_d_t_o_package(), &DesignerXdtoPackage, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR XDTOPackage → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("XDTOPackage", x_d_t_o_package(), &DesignerXdtoPackage, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/XDTOPackage. Имя `HARNESS_ENTRY` — контракт `build.rs`.
///
/// [`CorpusLayout::TextSidecar`]: дескриптор `<Name>.xml` (FilePerObject), текстовый спутник
/// — `<Name>/Ext/Package.bin` (`sidecar_inner="Ext/Package.bin"`,
/// `sidecar_format=DesignerXdto`). Testkit округляет ОБА byte-exact и сверяет схему для X.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "XDTOPackage",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/XDTOPackages",
    layout: formats_xml::CorpusLayout::TextSidecar {
        descriptor_ext: "xml",
        descriptor_dir_per_object: false,
        sidecar_inner: "Ext/Package.bin",
        sidecar_format: formats_xml::SidecarFormat::DesignerXdto,
    },
};
