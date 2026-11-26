//! Форматные примитивы EDT (`formats/*/src/common`, ARCHITECTURE.md §5): байтовая
//! обёртка (`envelope`) и константы корня дескриптора `.mdo`.
//!
//! Эти значения СВЕРЕНЫ hexdump'ом реального SSL-корпуса (556 файлов):
//! * нет BOM (первые байты `3c 3f 78` = `<?x`);
//! * EOL = `\r\n` (CRLF) — ВЕЗДЕ, включая отступы и хвост (бриф ошибочно говорил LF);
//! * отступ = 2 пробела на уровень;
//! * пролог `<?xml version="1.0" encoding="UTF-8"?>`;
//! * хвост: завершающий CRLF после `</mdclass:CommonModule>`.

use formats_xml::Envelope;

/// Точные байты XML-декларации EDT (без EOL).
pub const EDT_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// Префикс ns корня EDT-метаданных.
pub const MDCLASS_PREFIX: &str = "mdclass";

/// URI ns `mdclass` (атрибут `xmlns:mdclass`).
pub const MDCLASS_NS_URI: &str = "http://g5.1c.ru/v8/dt/metadata/mdclass";

/// Имя атрибута объявления ns корня (`xmlns:mdclass`).
pub const XMLNS_MDCLASS_ATTR: &str = "xmlns:mdclass";

/// Имя атрибута идентичности объекта на корне (`uuid`).
pub const UUID_ATTR: &str = "uuid";

/// Имя root-атрибута узла-идентичности плана обмена (`thisNode`). Несут лишь виды с
/// узлом обмена (ExchangePlan); прочие — без него.
pub const THIS_NODE_ATTR: &str = "thisNode";

/// Local-name элемента имени объекта (каркасное поле, не из спека).
pub const NAME_ELEMENT: &str = "name";

/// Envelope-константы EDT `.mdo` (сверены по корпусу).
pub const EDT_ENVELOPE: Envelope = Envelope {
    bom: false,
    eol: "\r\n",
    indent_unit: "  ",
    decl: EDT_DECL,
    trailing_eol: true,
    // EDT `.mdo` оставляет `>` ЛИТЕРАЛЬНЫМ в тексте (сверено: `Отлично (>=0.95)`).
    escape_gt: false,
    // EDT `.mdo` ЭКРАНИРУЕТ `"`→`&quot;` в тексте (сверено: `признак &quot;Рассмотрено&quot;`).
    escape_quot: true,
    // EDT разворачивает in-text перевод строки в `\r\n` (сверено: многостр. explanation).
    text_eol: "\r\n",
};
