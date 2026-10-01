//! EDT-проекция вида `WebSocketClient` (зеркало `core/spec/metadata/web_socket_client.rs`,
//! ARCHITECTURE.md §5). ВЫВОДИТСЯ из канонического спека (§2.2): тег = имя поля verbatim,
//! кодек = по value_kind. `headers` (ValueKind::List) не деривируем ⇒ auto-derive его НЕ
//! проецирует — в EDT-корпусе `<headers>` отсутствует (пустой ValueList), поэтому EDT его и
//! не читает/пишет (сжатие в дефолт `[]` даёт X-равенство с Designer/cf).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции EDT для `WebSocketClient` — деривация из спека.
pub struct EdtWebSocketClient;

impl LocusMap for EdtWebSocketClient {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, web_socket_client(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::web_socket_client::web_socket_client;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("WebSocketClient", web_socket_client(), &EdtWebSocketClient, bytes)
        .map_err(|e| e.to_string())
}

fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("WebSocketClient", web_socket_client(), &EdtWebSocketClient, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/WebSocketClient. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "WebSocketClient",
    read,
    write,
    corpus_subpath: "coverage/edt/s9_new/src/WebSocketClients",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
