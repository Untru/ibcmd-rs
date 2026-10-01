//! EDT-проекция вида `ScheduledJob` (зеркало `core/spec/metadata/scheduled_job.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)` (EDT SPARSE — дефолты опущены):
//! * `synonym` → `<synonym>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>` (`PlainText`);
//! * `methodName` → `<methodName>` (`PlainText`);
//! * `description` → `<description>` (`PlainText`);
//! * `key` → `<key>` (`PlainText`);
//! * `use` → `<use>true</use>` (`BoolPresence`; дефолт false опущен);
//! * `predefined` → `<predefined>true</predefined>` (`BoolPresence`);
//! * `restartCountOnFailure` → `<restartCountOnFailure>N</…>` (`IntText`; дефолт 0 опущен);
//! * `restartIntervalOnFailure` → `<restartIntervalOnFailure>N</…>` (`IntText`).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Edt для `ScheduledJob` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtScheduledJob;

impl LocusMap for EdtScheduledJob {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, scheduled_job(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::scheduled_job::scheduled_job;

/// R-read для харнесса: `.mdo`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ScheduledJob", scheduled_job(), &EdtScheduledJob, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ScheduledJob", scheduled_job(), &EdtScheduledJob, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/ScheduledJob. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "ScheduledJob",
    read,
    write,
    corpus_subpath: "coverage/edt/s6_misc/src/ScheduledJobs",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
