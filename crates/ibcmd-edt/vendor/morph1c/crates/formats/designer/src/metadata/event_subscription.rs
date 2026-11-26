//! Designer-проекция вида `EventSubscription` (зеркало
//! `core/spec/metadata/event_subscription.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec` (§1.6). Это ИНОЙ синтаксис того же
//! спека, что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<EventSubscription>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `source` → `<Source>` (`Type(TypeDialect::Designer)`): набор `<v8:Type>cfg:Kind.Имя`
//!   (концретный) ЛИБО `<v8:TypeSet>cfg:Kind`/`cfg:DefinedType.Имя` (type-set) — решение
//!   детерминировано из canon id кодеком;
//! * `event` → `<Event>литерал</Event>` (`EnumText`);
//! * `handler` → `<Handler>CommonModule.X.Y</Handler>` (`PlainText`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`v8:Type`/`v8:TypeSet`) несут v8 — это разбирает кодек.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `EventSubscription` — ВЫВОДИТСЯ из канонического
/// спека (`docs/APPROACH.md` §2.2): тег = [Kind, "Properties", UpperCamel(имя)], кодек = по value_kind. Список
/// полей/порядок/дефолты держит `core/spec` (§1.6); карта делегирует деривации
/// (`formats_xml::derive`) — заменяет рукописную таблицу `FieldId → (тег, кодек)`.
pub struct DesignerEventSubscription;

impl LocusMap for DesignerEventSubscription {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, event_subscription(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::event_subscription::event_subscription;

/// R-read для харнесса: `.xml`-байты EventSubscription → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("EventSubscription", event_subscription(), &DesignerEventSubscription, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR EventSubscription → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("EventSubscription", event_subscription(), &DesignerEventSubscription, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/EventSubscription. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "EventSubscription",
    read,
    write,
    corpus_subpath: "coverage/designer/s8_linked/EventSubscriptions",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
