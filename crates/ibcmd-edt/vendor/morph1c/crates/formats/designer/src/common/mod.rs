//! Форматные примитивы Designer-XML (`formats/*/src/common`, ARCHITECTURE.md §5):
//! байтовая обёртка (`envelope`) и КОНСТАНТЫ дескриптора `.xml`.
//!
//! Все значения СВЕРЕНЫ hexdump'ом реальных корпусов:
//! * есть UTF-8 BOM (первые байты `ef bb bf`);
//! * EOL = `\r\n` (CRLF) — ВЕЗДЕ;
//! * отступ = ОДИН TAB (`\t`) на уровень;
//! * пролог `<?xml version="1.0" encoding="UTF-8"?>`;
//! * НЕТ завершающего EOL после `</MetaDataObject>` (последние байты — `t>`);
//! * корень `<MetaDataObject>` с дефолтным `xmlns` + N префиксных `xmlns:*` +
//!   атрибут `version="…"`.
//!
//! # Версионность обёртки (FORMATS.md §1/§2/§3)
//!
//! Версия формата выгрузки — ФУНКЦИЯ версии платформы, а НЕ свойство конфигурации
//! (FORMATS.md §1). Одна и та же конфигурация, выгруженная разными платформами, даёт
//! РАЗНЫЙ envelope. Конкретно ns-блок и значение `version=` расходятся по версиям:
//!
//! | Корпус | Платформа | `version=` | ns-блок |
//! |---|---|---|---|
//! | SSL | 8.5.1  | `2.21` | 18 xmlns (ВКЛ. `xmlns:pal`) — сверено по 556 файлам |
//! | ERP | 8.3.27 | `2.20` | 17 xmlns (БЕЗ `xmlns:pal`) — сверено по корпусу ERP |
//!
//! Поэтому обёртка НЕ хардкодит один корпус: она — РЕЕСТР `{версия_формата →
//! (значение_version, ns-блок)}` ([`ENVELOPE_PROFILES`]), ключёванный
//! [`morph1c_core::version::FormatVersion`]. Значение `version=` НЕ дублируется как
//! строка — оно ПРОИЗВОДИТСЯ из `FormatVersion` (`2.20`/`2.21` = `FormatVersion::to_string`),
//! а сама `FormatVersion` берётся из таблицы FORMATS.md §2 (`core::version`). Так код
//! синхронизируется С FORMATS.md, а не заводит второй хардкод.
//!
//! Ридер ДЕТЕКТИТ версию из корневого `version=` ([`detect_profile`]) и сверяет РОВНО
//! её ns-блок; писатель ЭМИТИТ ns-блок таргет-версии ([`profile_for`]). Неизвестная
//! версия / несовпадение ns — жёсткая ошибка (§1.0: не «молча исправить», не дефолт).

use std::sync::OnceLock;

use formats_xml::{Element, Envelope, OutElement};
use morph1c_core::version::FormatVersion;

/// Точные байты XML-декларации Designer (без EOL) — те же, что у EDT.
pub const DESIGNER_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// Local-name корня Designer-дескриптора.
pub const ROOT_ELEMENT: &str = "MetaDataObject";

// NB: local-name обёртки вида (`<CommonModule>`, `<Catalog>`, …) НЕ константа — это
// канонический код вида (`obj.kind` / `EntitySpec.entity`), который kind-параметризованный
// путь коннектора (`read_descriptor`/`write_descriptor`) берёт как параметр.

/// Local-name контейнера свойств.
pub const PROPERTIES_ELEMENT: &str = "Properties";

/// Local-name элемента имени объекта (каркасное поле, не из спека); под `<Properties>`.
pub const NAME_ELEMENT: &str = "Name";

/// Имя атрибута идентичности объекта на `<CommonModule>` (`uuid`).
pub const UUID_ATTR: &str = "uuid";

/// Имя атрибута версии формата на корне (`version`).
pub const VERSION_ATTR: &str = "version";

/// Имя атрибута объявления дефолтного ns (`xmlns`).
pub const XMLNS_DEFAULT_ATTR: &str = "xmlns";

/// URI дефолтного ns корня (`xmlns="…/MDClasses"`).
pub const MDCLASSES_NS_URI: &str = "http://v8.1c.ru/8.3/MDClasses";

/// Префикс ns `v8` (используется синонимом: `v8:item/v8:lang/v8:content`).
pub const V8_PREFIX: &str = "v8";

/// ns-блок корня корпуса **SSL 8.5.1** (формат 2.21) в ТОЧНОМ порядке эталона:
/// дефолтный `xmlns`, затем 17 префиксных `xmlns:*` (app … xsi) ВКЛЮЧАЯ `xmlns:pal`
/// (18 xmlns всего). `(имя_атрибута, URI)`. Сверено: блок байт-идентичен по всем 556
/// файлам SSL CommonModule (`uniq` → одна строка).
///
/// БОЛЬШИНСТВО префиксов реально не используется дескриптором → но 1С эмитит весь блок
/// (§3.2). Ридер обязан сверить КАЖДЫЙ (claim+equality), иначе ошибка (§1.0).
pub const NS_BLOCK_SSL: &[(&str, &str)] = &[
    ("xmlns", "http://v8.1c.ru/8.3/MDClasses"),
    ("xmlns:app", "http://v8.1c.ru/8.2/managed-application/core"),
    (
        "xmlns:cfg",
        "http://v8.1c.ru/8.1/data/enterprise/current-config",
    ),
    ("xmlns:cmi", "http://v8.1c.ru/8.2/managed-application/cmi"),
    ("xmlns:ent", "http://v8.1c.ru/8.1/data/enterprise"),
    (
        "xmlns:lf",
        "http://v8.1c.ru/8.2/managed-application/logform",
    ),
    ("xmlns:pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    ("xmlns:xen", "http://v8.1c.ru/8.3/xcf/enums"),
    ("xmlns:xpr", "http://v8.1c.ru/8.3/xcf/predef"),
    ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

/// ns-блок корня корпуса **ERP 8.3.27** (формат 2.20) в ТОЧНОМ порядке эталона: тот
/// же порядок, что у [`NS_BLOCK_SSL`], но БЕЗ строки `xmlns:pal` (17 xmlns всего).
/// `(имя_атрибута, URI)`. Сверено по корпусу ERP (CommonModule/Constant/…): 17 xmlns,
/// нет `pal`, `version="2.20"`; прочие 17 xmlns байт-идентичны SSL и в том же порядке.
///
/// Единственное отличие от SSL — отсутствие `pal` (палитра цветов) + значение
/// `version`. Тест [`tests::erp_ns_block_is_ssl_without_pal`] держит это соотношение.
pub const NS_BLOCK_ERP: &[(&str, &str)] = &[
    ("xmlns", "http://v8.1c.ru/8.3/MDClasses"),
    ("xmlns:app", "http://v8.1c.ru/8.2/managed-application/core"),
    (
        "xmlns:cfg",
        "http://v8.1c.ru/8.1/data/enterprise/current-config",
    ),
    ("xmlns:cmi", "http://v8.1c.ru/8.2/managed-application/cmi"),
    ("xmlns:ent", "http://v8.1c.ru/8.1/data/enterprise"),
    (
        "xmlns:lf",
        "http://v8.1c.ru/8.2/managed-application/logform",
    ),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    ("xmlns:xen", "http://v8.1c.ru/8.3/xcf/enums"),
    ("xmlns:xpr", "http://v8.1c.ru/8.3/xcf/predef"),
    ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

/// Фиксированный ns-блок корня для корпуса SSL 8.5.1. **Совместимость**: исторический
/// синоним [`NS_BLOCK_SSL`] — путь коннектора `lib.rs` продолжает ссылаться на это имя
/// (SSL остаётся byte-exact без изменений). Для новых версий выбирайте блок через
/// [`profile_for`]/[`detect_profile`].
pub const ROOT_NS_BLOCK: &[(&str, &str)] = NS_BLOCK_SSL;

/// Значение `version` для корпуса SSL (формат 2.21). **Совместимость**: исторический
/// синоним `profile_for(SSL).version_value`; `lib.rs` продолжает ссылаться на это имя.
/// Производится из [`SSL`] (`FormatVersion` таблицы FORMATS.md §2) — не второй хардкод
/// (тест [`tests::profile_version_value_matches_format`] держит равенство `to_string`).
pub const VERSION_VALUE: &str = "2.21";

/// Один профиль обёртки: версия формата → (значение `version=`, ns-блок корня).
///
/// `version_value` НЕ хранит второй хардкод строки — он равен `format.to_string()`
/// (`FormatVersion` таблицы FORMATS.md §2). Держится const-строкой лишь потому, что
/// `to_string` не const; тест [`tests::profile_version_value_matches_format`] сверяет
/// равенство, замыкая их к FORMATS.md.
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeProfile {
    /// Версия формата выгрузки (ключ реестра; из `core::version`, FORMATS.md §2).
    pub format: FormatVersion,
    /// Точное значение атрибута `version=` корня (`"2.20"`/`"2.21"`), == `format.to_string()`.
    pub version_value: &'static str,
    /// ns-блок корня в ТОЧНОМ порядке эталона для этой версии.
    pub ns_block: &'static [(&'static str, &'static str)],
}

/// Имя конструкции «корневая обёртка Designer-дескриптора» в таблице раскладок
/// (`models/format_layouts.jsonl`): ростер ns и версии, в которых он наблюдался.
pub const DESCRIPTOR_ENVELOPE_CONSTRUCT: &str = "designer.descriptor.envelope";

/// Реестр обёрток Designer-XML `{версия_формата → профиль}` — ОТВЕТ ДАННЫХ.
///
/// Ни версии, ни ns-ростеры здесь не записаны: и то и другое снято с корней РЕАЛЬНЫХ дампов
/// одной и той же конфигурации, собранной каждой доступной платформой
/// (`tools/build_format_layouts.py`; в 2.17 и 2.20 ростер один и тот же — 17 объявлений без
/// `pal`, в 2.21 их 18). Реестр расширяется появлением НОВОГО ДАМПА, а не строкой в коде.
///
/// Версия, дампа которой нет, в реестр не попадает — и неизвестный `version=` по-прежнему
/// громкая ошибка (§1.0: угаданная обёртка молча дала бы дамп не той версии).
///
/// Значение `version=` производно от `format` (`FormatVersion::to_string`), поэтому вторым
/// хардкодом оно тут не лежит; строка живёт столько же, сколько процесс (профилей единицы,
/// строятся один раз).
pub fn envelope_profiles() -> &'static [EnvelopeProfile] {
    static PROFILES: OnceLock<Vec<EnvelopeProfile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        let mut rows: Vec<&'static morph1c_core::version::Layout> =
            morph1c_core::version::layout::rows(DESCRIPTOR_ENVELOPE_CONSTRUCT);
        rows.sort_by_key(|l| l.version());
        rows.into_iter()
            .map(|l| EnvelopeProfile {
                format: l.version(),
                version_value: Box::leak(l.version().to_string().into_boxed_str()),
                ns_block: l.ns,
            })
            .collect()
    })
}

/// Профиль обёртки для заданной версии формата (таргет писателя), если версия
/// известна реестру. Неизвестная версия ⇒ `None` (коннектор → жёсткая ошибка, §1.0:
/// не дефолтить молча).
pub fn profile_for(format: FormatVersion) -> Option<&'static EnvelopeProfile> {
    envelope_profiles().iter().find(|p| p.format == format)
}

/// Детект профиля обёртки по значению корневого `version=` (строка из дескриптора,
/// напр. `"2.20"`). Сопоставляет строку с `version_value` реестра. Неизвестное/битое
/// значение ⇒ `None` (ридер → жёсткая ошибка, §1.0). Это версионный вход РИДЕРА:
/// формат ВХОДА детектится (FORMATS.md §1).
pub fn detect_profile(version_value: &str) -> Option<&'static EnvelopeProfile> {
    envelope_profiles()
        .iter()
        .find(|p| p.version_value == version_value)
}

/// Сверить + claim ВСЮ версионную часть корневой обёртки (`version=` + ns-блок) РИДЕРА,
/// вернуть детектированную версию формата (FORMATS.md §1: формат входа детектится).
///
/// Механизм (версия-осознанный, без хардкода одного корпуса):
/// 1. читаем корневой `version=` → [`detect_profile`] выбирает профиль реестра; иначе
///    ошибка (неизвестная версия — не дефолт, §1.0);
/// 2. для КАЖДОГО `(имя, URI)` ns-блока ЭТОЙ версии: атрибут ОБЯЗАН присутствовать и
///    быть равен URI (иначе ошибка), затем claim'ится;
/// 3. `version=` claim'ится.
///
/// Лишний ns-атрибут (например `xmlns:pal` в ERP-файле) НЕ claim'ится здесь → он
/// останется невостребованным и упрётся в тотальность (`leftover != 0`, §1.0) выше по
/// коннектору — то есть чужой блок отвергается, а не проглатывается.
///
/// Возвращает `Err(String)` с человекочитаемой причиной для `DesignerError::Envelope`.
pub fn verify_root_envelope(root: &Element) -> Result<FormatVersion, String> {
    let version = root
        .attr(VERSION_ATTR)
        .ok_or_else(|| format!("missing root @{VERSION_ATTR}"))?;
    let profile = detect_profile(&version.value).ok_or_else(|| {
        format!(
            "unrecognized Designer format version {:?} (not in FORMATS.md registry)",
            version.value
        )
    })?;

    for (name, want_uri) in profile.ns_block {
        let a = root
            .attr(name)
            .ok_or_else(|| format!("missing root attribute {name} (format {})", profile.format))?;
        if a.value != *want_uri {
            return Err(format!("root {name} = {:?}, want {want_uri:?}", a.value));
        }
        a.claimed.set(true);
    }
    version.claimed.set(true);
    Ok(profile.format)
}

/// Построить корневой `<MetaDataObject>` с ns-блоком + `version=` ТАРГЕТ-версии (для
/// ПИСАТЕЛЯ; FORMATS.md §1: формат выхода — параметр). Неизвестная версия ⇒ `Err` (не
/// молчаливый дефолт, §1.0).
///
/// Возвращает готовый `OutElement` корня (без детей) — коннектор дописывает `<<kind>>`.
pub fn emit_root_envelope(format: FormatVersion) -> Result<OutElement, String> {
    let profile = profile_for(format).ok_or_else(|| {
        format!("no Designer envelope profile for format {format} (not in FORMATS.md registry)")
    })?;
    let mut root = OutElement::branch("", ROOT_ELEMENT);
    for (name, uri) in profile.ns_block {
        root = root.attr(*name, *uri);
    }
    root = root.attr(VERSION_ATTR, profile.version_value);
    Ok(root)
}

/// Envelope-константы Designer `.xml` (сверены по корпусу). Отличия от EDT:
/// `bom=true`, `indent_unit="\t"`, `trailing_eol=false`. Байтовая обёртка
/// (BOM/EOL/indent/пролог) ОДНА для всех версий Designer — версионны лишь ns-блок и
/// значение `version=` (см. [`ENVELOPE_PROFILES`]).
pub const DESIGNER_ENVELOPE: Envelope = Envelope {
    bom: true,
    eol: "\r\n",
    indent_unit: "\t",
    decl: DESIGNER_DECL,
    trailing_eol: false,
    // Designer `.xml` ЭКРАНИРУЕТ `>` → `&gt;` в тексте (сверено: `Отлично (&gt;=0.95)`).
    escape_gt: true,
    // Designer `.xml` оставляет `"` ЛИТЕРАЛЬНОЙ в тексте (сверено: `признак "Рассмотрено"`).
    escape_quot: false,
    // Designer хранит in-text перевод строки как `\n` (сверено: многостр. Explanation).
    text_eol: "\n",
};

#[cfg(any())]
mod tests;
