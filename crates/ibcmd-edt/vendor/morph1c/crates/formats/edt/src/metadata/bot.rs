//! EDT-проекция вида `Bot` (зеркало `core/spec/metadata/bot.rs`, ARCHITECTURE.md §5).
//! ВЫВОДИТСЯ из канонического спека (`docs/APPROACH.md` §2.2): тег = имя поля verbatim,
//! кодек = по value_kind. Список полей/порядок/дефолты держит `core/spec` (§1.6).
//! `picture` (empty Str) в EDT sparse НЕ эмитится; `headers`/List у Bot нет.

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции EDT для `Bot` — деривация из спека.
pub struct EdtBot;

impl LocusMap for EdtBot {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, bot(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::bot::bot;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Bot", bot(), &EdtBot, bytes).map_err(|e| e.to_string())
}

fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Bot", bot(), &EdtBot, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Bot. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Bot",
    read,
    write,
    corpus_subpath: "coverage/edt/s9_new/src/Bots",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
