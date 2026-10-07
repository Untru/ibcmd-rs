//! EDT-проекция вида `XDTOPackage` (зеркало `core/spec/metadata/x_d_t_o_package.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек ТОНКОГО дескриптора — канонику
//! (id/порядок/дефолты/нормализацию) держит `core/spec` (§1.6).
//!
//! # Дескриптор `.mdo` — тонкий; схема XDTO — текстовый СПУТНИК
//! EDT `.mdo` XDTOPackage несёт лишь non-default тонкие поля: `<synonym>` (все 54 SSL),
//! `<comment>` (дефолт `""` → EDT опускает, разрежённо) и `<namespace>` (у всех). Сама СХЕМА
//! (рекурсивное дерево типов) — ОТДЕЛЬНЫЙ текстовый XML-файл-СПУТНИК `Package.xdto` в том же
//! каталоге объекта, НЕ элемент `.mdo`: он не проецируется этой картой, а фреймится
//! sidecar-aware путём харнесса ([`CorpusLayout::TextSidecar`] + XDTO-schema-кодек
//! `formats_xml::xdto` + testkit).
//!
//! Карта `FieldId → (XmlLocus, Codec)` — плоские дети корня `<mdclass:XDTOPackage>` (`path`
//! длины 1, ns=""), как у CommonModule/Role. Порядок в файле задаёт ПОРЯДОК СПЕКА.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Edt для `XdtoPackage` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtXdtoPackage;

impl LocusMap for EdtXdtoPackage {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, x_d_t_o_package(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---
//
// `read`/`write` — ДЕСКРИПТОР `.mdo` (тонкий) byte-exact. Текстовый спутник `Package.xdto`
// округляется testkit'ом по `CorpusLayout::TextSidecar` (спутник путешествует с объектом,
// round-trip byte-exact через `formats_xml::xdto::{read,write}` c `SidecarFormat::EdtXdto`).
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::x_d_t_o_package::x_d_t_o_package;

/// R-read для харнесса: `.mdo`-байты XDTOPackage → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("XDTOPackage", x_d_t_o_package(), &EdtXdtoPackage, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR XDTOPackage → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("XDTOPackage", x_d_t_o_package(), &EdtXdtoPackage, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/XDTOPackage. Имя `HARNESS_ENTRY` — контракт `build.rs`.
///
/// [`CorpusLayout::TextSidecar`]: дескриптор `<Name>/<Name>.mdo` (DirPerObject), текстовый
/// спутник — СИБЛИНГ `<Name>/Package.xdto` (`sidecar_inner="Package.xdto"`,
/// `sidecar_format=EdtXdto`). Testkit округляет ОБА byte-exact и сверяет схему для X.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "XDTOPackage",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/XDTOPackages",
    layout: formats_xml::CorpusLayout::TextSidecar {
        descriptor_ext: "mdo",
        descriptor_dir_per_object: true,
        sidecar_inner: "Package.xdto",
        sidecar_format: formats_xml::SidecarFormat::EdtXdto,
    },
};
