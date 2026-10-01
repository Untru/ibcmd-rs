//! EDT-проекция вида `CommonModule` (зеркало `core/spec/metadata/common_module.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику (id/kind/порядок/
//! дефолты/нормализацию) держит `core/spec` (§1.6).
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>lang</key><value>text</value>`
//!   (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`);
//! * 7 bool-флагов → presence-only `<flag>true</flag>` (`BoolPresence`);
//! * `returnValuesReuse` → `<returnValuesReuse>Literal</returnValuesReuse>` (`EnumText`).
//!
//! Все 10 — плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок
//! детей в файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), а карта — лишь имена.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции EDT для `CommonModule` — ВЫВОДИТСЯ из канонического спека
/// (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); эта карта лишь делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtCommonModule;

impl LocusMap for EdtCommonModule {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, common_module(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---
//
// Type-erased обёртки (ошибка → строкой) над kind-параметризованным путём коннектора.
// `HARNESS_ENTRY` локальна ЭТОМУ файлу — добавление вида не правит общих файлов (§5).
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_module::common_module;

/// R-read для харнесса: `.mdo`-байты CommonModule → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonModule", common_module(), &EdtCommonModule, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonModule → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonModule", common_module(), &EdtCommonModule, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/CommonModule. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CommonModule",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/CommonModules",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
