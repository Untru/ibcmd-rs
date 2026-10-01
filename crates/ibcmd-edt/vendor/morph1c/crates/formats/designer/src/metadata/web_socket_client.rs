//! Designer-проекция вида `WebSocketClient` (зеркало `core/spec/metadata/web_socket_client.rs`,
//! ARCHITECTURE.md §5). ВЫВОДИТСЯ из канонического спека (§2.2) для всех скалярных полей;
//! `headers` (ValueKind::List) не деривируем ⇒ ЯВНО маппится на [`Codec::EmptyValueList`]
//! (`<Headers xsi:type="xr:ValueList"/>`, DENSE пустой список). Иной синтаксис ТОГО ЖЕ спека,
//! что EDT → равный IR (§3.5).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::web_socket_client::{web_socket_client, F_HEADERS};

/// Путь Designer до `<Headers>` (от корня `<MetaDataObject>` через `<WebSocketClient>/
/// <Properties>`). `headers` — ValueKind::List, поэтому auto-derive его не проецирует.
const PP_HEADERS: &[&str] = &["WebSocketClient", "Properties", "Headers"];

/// Карта проекции Designer для `WebSocketClient`: скаляры — деривация из спека; `headers`
/// — ручной `Codec::EmptyValueList`.
pub struct DesignerWebSocketClient;

impl LocusMap for DesignerWebSocketClient {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == F_HEADERS {
            return Some(FieldProjection::new(
                XmlLocus::PropElement { path: PP_HEADERS, ns: "" },
                Codec::EmptyValueList,
            ));
        }
        projection(DeriveDialect::Designer, web_socket_client(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("WebSocketClient", web_socket_client(), &DesignerWebSocketClient, bytes)
        .map_err(|e| e.to_string())
}

fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("WebSocketClient", web_socket_client(), &DesignerWebSocketClient, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/WebSocketClient. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "WebSocketClient",
    read,
    write,
    corpus_subpath: "coverage/designer/s9_new/WebSocketClients",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
