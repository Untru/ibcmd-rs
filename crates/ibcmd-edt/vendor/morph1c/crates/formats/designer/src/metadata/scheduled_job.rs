//! Designer-проекция вида `ScheduledJob` (зеркало `core/spec/metadata/scheduled_job.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`
//! (§1.6). Это ИНОЙ синтаксис того же спека, что и EDT, поэтому оба формата дают РАВНЫЙ
//! IR (§3.5). Designer DENSE — все поля present всегда (дефолты эмитятся: `<Comment/>`,
//! `<Key/>`, `<Use>false</Use>`, `<RestartCountOnFailure>0</…>` …).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<ScheduledJob>/<Properties>`:
//! * `synonym` → `<Synonym>` (`LocalizedV8`);
//! * `comment` → `<Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `methodName` → `<MethodName>` (`PlainText`);
//! * `description` → `<Description>` (`PlainText`; пустой → `<Description/>`);
//! * `key` → `<Key>` (`PlainText`; пустой → `<Key/>`);
//! * `use` → `<Use>true|false</Use>` (`BoolText`);
//! * `predefined` → `<Predefined>true|false</Predefined>` (`BoolText`);
//! * `restartCountOnFailure` → `<RestartCountOnFailure>N</…>` (`IntText`);
//! * `restartIntervalOnFailure` → `<RestartIntervalOnFailure>N</…>` (`IntText`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `ScheduledJob` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerScheduledJob;

impl LocusMap for DesignerScheduledJob {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, scheduled_job(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::scheduled_job::scheduled_job;

/// R-read для харнесса: `.xml`-байты → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ScheduledJob", scheduled_job(), &DesignerScheduledJob, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ScheduledJob", scheduled_job(), &DesignerScheduledJob, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/ScheduledJob. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "ScheduledJob",
    read,
    write,
    corpus_subpath: "coverage/designer/s6_misc/ScheduledJobs",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
