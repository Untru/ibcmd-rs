//! EDT-проекция вида `Role` (зеркало `core/spec/metadata/role.rs`, ARCHITECTURE.md §5).
//! ТОЛЬКО размещение/кодировка ячеек ТОНКОГО дескриптора — канонику (id/порядок/дефолты/
//! нормализацию) держит `core/spec` (§1.6).
//!
//! # Дескриптор `.mdo` — тонкий; таблица прав — текстовый СПУТНИК
//! EDT `.mdo` Role несёт лишь non-default тонкие поля: `<synonym>` (все 107 SSL) и
//! `<comment>` (дефолт `""` → EDT опускает, разрежённо). Сама ТАБЛИЦА ПРАВ — ОТДЕЛЬНЫЙ
//! текстовый XML-файл-СПУТНИК `Rights.rights` в том же каталоге объекта, НЕ элемент `.mdo`:
//! он не проецируется этой картой, а фреймится sidecar-aware путём харнесса
//! ([`CorpusLayout::TextSidecar`] + rights-table-кодек `formats_xml::rights` + testkit).
//!
//! Карта `FieldId → (XmlLocus, Codec)` — плоские дети корня `<mdclass:Role>` (`path` длины
//! 1, ns=""), как у CommonModule. Порядок в файле задаёт ПОРЯДОК СПЕКА.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Edt для `Role` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtRole;

impl LocusMap for EdtRole {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, role(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---
//
// `read`/`write` — ДЕСКРИПТОР `.mdo` (тонкий) byte-exact. Текстовый спутник `Rights.rights`
// округляется testkit'ом по `CorpusLayout::TextSidecar` (спутник путешествует с объектом,
// round-trip byte-exact через `formats_xml::rights::{read,write}` c `SidecarFormat::EdtRights`).
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::role::role;

/// R-read для харнесса: `.mdo`-байты Role → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Role", role(), &EdtRole, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Role → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Role", role(), &EdtRole, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Role. Имя `HARNESS_ENTRY` — контракт `build.rs`.
///
/// [`CorpusLayout::TextSidecar`]: дескриптор `<Name>/<Name>.mdo` (DirPerObject), текстовый
/// спутник — СИБЛИНГ `<Name>/Rights.rights` (`sidecar_inner="Rights.rights"`,
/// `sidecar_format=EdtRights`). Testkit округляет ОБА byte-exact и сверяет таблицу прав для X.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Role",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/Roles",
    layout: formats_xml::CorpusLayout::TextSidecar {
        descriptor_ext: "mdo",
        descriptor_dir_per_object: true,
        sidecar_inner: "Rights.rights",
        sidecar_format: formats_xml::SidecarFormat::EdtRights,
    },
};
