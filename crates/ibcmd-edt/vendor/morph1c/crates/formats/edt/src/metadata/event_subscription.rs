//! EDT-проекция вида `EventSubscription` (зеркало
//! `core/spec/metadata/event_subscription.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6).
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>lang</key><value>text</value>`
//!   (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `source` → `<source>` (`Type(TypeDialect::Edt)`): набор `<types>id</types>` —
//!   bare ref-kind / `Kind.Имя` / `DefinedType.Имя`;
//! * `event` → `<event>литерал</event>` (`EnumText`);
//! * `handler` → `<handler>CommonModule.X.Y</handler>` (`PlainText`).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Edt для `EventSubscription` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim, кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct EdtEventSubscription;

impl LocusMap for EdtEventSubscription {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, event_subscription(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::event_subscription::event_subscription;

/// R-read для харнесса: `.mdo`-байты EventSubscription → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("EventSubscription", event_subscription(), &EdtEventSubscription, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR EventSubscription → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("EventSubscription", event_subscription(), &EdtEventSubscription, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/EventSubscription. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "EventSubscription",
    read,
    write,
    corpus_subpath: "coverage/edt/s8_linked/src/EventSubscriptions",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
