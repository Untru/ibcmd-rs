//! Designer-проекция вида `Role` (зеркало `core/spec/metadata/role.rs`, ARCHITECTURE.md
//! §5). ТОЛЬКО размещение/кодировка ячеек ТОНКОГО дескриптора — канонику (id/порядок/
//! дефолты/нормализацию) держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT
//! → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! # Раскладка Designer role (главный дескриптор + текстовый спутник прав)
//! * главный `<Name>.xml` — тонкий дескриптор (эта карта): DENSE (эмитятся и дефолты —
//!   `<Comment/>`).
//! * `<Name>/Ext/Rights.xml` — текстовый спутник таблицы прав (ns `…/8.2/roles`, root
//!   `<Rights version="2.21">`); round-trip byte-exact через rights-table-кодек
//!   (`formats_xml::rights` c `SidecarFormat::DesignerRights`; testkit TextSidecar).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от корня `<MetaDataObject>` через
//! `<Role>/<Properties>` (UpperCamelCase, property-теги без префикса, ns="").

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `Role` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerRole;

impl LocusMap for DesignerRole {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, role(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::role::role;

/// R-read для харнесса: `.xml`-байты Role → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Role", role(), &DesignerRole, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Role → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Role", role(), &DesignerRole, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Role. Имя `HARNESS_ENTRY` — контракт `build.rs`.
///
/// [`CorpusLayout::TextSidecar`]: дескриптор `<Name>.xml` (FilePerObject), текстовый спутник
/// — `<Name>/Ext/Rights.xml` (`sidecar_inner="Ext/Rights.xml"`, `sidecar_format=DesignerRights`).
/// Testkit округляет ОБА byte-exact и сверяет таблицу прав для X.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Role",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/Roles",
    layout: formats_xml::CorpusLayout::TextSidecar {
        descriptor_ext: "xml",
        descriptor_dir_per_object: false,
        sidecar_inner: "Ext/Rights.xml",
        sidecar_format: formats_xml::SidecarFormat::DesignerRights,
    },
};
