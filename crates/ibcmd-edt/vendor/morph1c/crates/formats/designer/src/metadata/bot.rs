//! Designer-проекция вида `Bot` (зеркало `core/spec/metadata/bot.rs`, ARCHITECTURE.md §5).
//! ВЫВОДИТСЯ из канонического спека (§2.2): тег = `[Kind, "Properties", UpperCamel(имя)]`,
//! кодек = по value_kind. DENSE: `<Predefined>…</Predefined>`, пустой `picture` → `<Picture/>`
//! (PlainText self-closing). Иной синтаксис ТОГО ЖЕ спека, что EDT → равный IR (§3.5).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;

/// Карта проекции Designer для `Bot` — деривация из спека.
pub struct DesignerBot;

impl LocusMap for DesignerBot {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, bot(), field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::bot::bot;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Bot", bot(), &DesignerBot, bytes).map_err(|e| e.to_string())
}

fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Bot", bot(), &DesignerBot, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Bot. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Bot",
    read,
    write,
    corpus_subpath: "coverage/designer/s9_new/Bots",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
