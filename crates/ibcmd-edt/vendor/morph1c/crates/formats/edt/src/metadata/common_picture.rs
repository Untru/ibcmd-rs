//! EDT-проекция вида `CommonPicture` (зеркало `core/spec/metadata/common_picture.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек ТОНКОГО дескриптора — канонику
//! (id/порядок/дефолты/нормализацию) держит `core/spec` (§1.6).
//!
//! # Дескриптор `.mdo` — тонкий; картинка — Blob-СПУТНИК
//! EDT `.mdo` несёт лишь non-default тонкие поля: `<synonym>` (все 600 SSL) и `<comment>`
//! (5 объектов). `availabilityForChoice`/`availabilityForAppearance` = дефолт `false` →
//! EDT их ОПУСКАЕТ (разрежённо). Сама КАРТИНКА — БИНАРНЫЙ файл-СПУТНИК `Picture.<ext>` в
//! том же каталоге объекта (§1.0 Blob), НЕ элемент `.mdo`: он не проецируется этой картой,
//! а фреймится sidecar-aware путём харнесса (`CorpusLayout::BinarySidecar` + testkit).
//!
//! Карта `FieldId → (XmlLocus, Codec)` — плоские дети корня `<mdclass:CommonPicture>`
//! (`path` длины 1, ns=""), как у CommonModule. Порядок в файле задаёт ПОРЯДОК СПЕКА.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_picture::F_TRANSPARENT_PIXEL;

/// Карта проекции Edt для `CommonPicture` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
///
/// Единственная рукописная строка — `transparentPixel` (ValueKind::List деривация не
/// выводит): EDT-узел `<transparentPixel><x>10</x><y>7</y></transparentPixel>` со
/// SPARSE-листьями (witness ERP `ВажностиНовостей` x/y; `ВажностьНовостиОченьВажная` —
/// только `<x>14</x>`, y=0 опущен). Designer-дескриптор узла НЕ несёт (носитель — обёртка
/// `Ext/Picture.xml`, читает `pipeline::picture_read`) — его derive-карта List-поле
/// пропускает сама.
pub struct EdtCommonPicture;

impl LocusMap for EdtCommonPicture {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == F_TRANSPARENT_PIXEL {
            return Some(FieldProjection::new(
                XmlLocus::PropElement { path: &["transparentPixel"], ns: "" },
                Codec::TransparentPixel,
            ));
        }
        projection(DeriveDialect::Edt, common_picture(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---
//
// `read`/`write` — ДЕСКРИПТОР `.mdo` (тонкий) byte-exact. Бинарный спутник `Picture.<ext>`
// округляется testkit'ом по `CorpusLayout::BinarySidecar` (спутник путешествует с объектом).
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_picture::common_picture;

/// R-read для харнесса: `.mdo`-байты CommonPicture → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonPicture", common_picture(), &EdtCommonPicture, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonPicture → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonPicture", common_picture(), &EdtCommonPicture, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/CommonPicture. Имя `HARNESS_ENTRY` — контракт `build.rs`.
///
/// [`CorpusLayout::BinarySidecar`]: дескриптор `<Name>/<Name>.mdo` (DirPerObject), бинарный
/// спутник — СИБЛИНГ `<Name>/Picture.<ext>` (`sidecar_subdir=""`). Testkit округляет ОБА
/// byte-exact и материализует Blob для X.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CommonPicture",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/CommonPictures",
    layout: formats_xml::CorpusLayout::BinarySidecar {
        descriptor_ext: "mdo",
        descriptor_dir_per_object: true,
        sidecar_subdir: "",
        sidecar_stem: "Picture",
        // EDT не имеет вспом. XML-дескриптора картинки (бинарь — прямой сиблинг `.mdo`).
        sidecar_xml_subdir: "",
    },
};
