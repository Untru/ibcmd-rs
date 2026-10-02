//! Rights-table-кодек: содержимое текстового XML-спутника `Rights.rights` (EDT) /
//! `Ext/Rights.xml` (Designer) ↔ канонический [`RightsTable`] (`core/spec/metadata/role`),
//! общий субстрат среза S2 (Rights/Role).
//!
//! # Зачем ЗДЕСЬ (formats-xml)
//! Таблица прав Role хранится ОТДЕЛЬНЫМ XML-файлом рядом с дескриптором (ns
//! `http://v8.1c.ru/8.2/roles`, root `<Rights>`): три флага + список `<object>` (полное
//! имя `Kind.Name` + `<right><name>…</name><value>true|false</value>`). И EDT, и Designer
//! держат ОДНО И ТО ЖЕ тело (сверено корпусом SSL: флаги/объекты/права байт-идентичны),
//! различаясь ЛИШЬ envelope-обёрткой (BOM / порядок ns / `version` / финальный перевод
//! строки). Поэтому парсинг тела — ОБЩИЙ, а envelope — параметр ([`SidecarFormat`]):
//! обе стороны дают РАВНУЮ [`RightsTable`] → cross-format X by construction (§1.6/§3.5), а
//! R остаётся byte-exact для КАЖДОГО формата (каждый регенерирует СВОЙ envelope).
//!
//! # §1.0-тотальность (не best-effort)
//! Парсер СТРОГ: он сверяет envelope байт-в-байт и разбирает тело детерминированным
//! рекурсивным спуском по ТОЧНОЙ форме (TAB-отступы, CRLF, порядок элементов). ЛЮБОЕ
//! отклонение (иной ns/флаг/элемент/отступ/незакрытый тег/не-bool value) → типизированная
//! [`RightsError`], а НЕ проглатывание. `emit(parse(x)) == x` byte-exact гарантируется тем,
//! что форма фиксирована и вся вариативность (значения флагов/имена/значения прав) захвачена
//! в [`RightsTable`]; текст имён переносится ВЕРБАТИМ (в корпусе XML-эскейпов нет — любой
//! `<`/`&` в тексте имени сорвал бы парс тега → ошибка, а не тихий best-effort).

use sha2::{Digest, Sha256};
use std::{borrow::Cow, sync::OnceLock};

use morph1c_core::spec::metadata::role::{
    RestrictionTemplate, Right, RightRestriction, RightsObject, RightsSourceLayout, RightsTable,
};
use morph1c_core::version::{FormatVersion, SSL};

use crate::registry::SidecarFormat;

/// Типизированная ошибка rights-table-кодека (§1.0 — без best-effort/skip).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RightsError(pub String);

impl std::fmt::Display for RightsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rights sidecar: {}", self.0)
    }
}

impl std::error::Error for RightsError {}

fn err<T>(msg: impl Into<String>) -> Result<T, RightsError> {
    Err(RightsError(msg.into()))
}

// --- Envelope-константы (сверено корпусом SSL 107/107) --------------------------------

const BOM: &[u8] = b"\xef\xbb\xbf";
const XML_DECL: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n";

/// Открывающий тег `<Rights …>` EDT (без BOM, без `version` — EDT-конверт версии НЕ несёт;
/// сверено: ERP-корпус `Rights.rights` байт-идентичен SSL-корпусному конверту).
const EDT_RIGHTS_OPEN: &[u8] =
    b"<Rights xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=\"http://v8.1c.ru/8.2/roles\" xsi:type=\"Rights\">\r\n";
/// Открывающий тег `<Rights …>` Designer (с BOM выше) ДО значения `version=`.
///
/// Версионна ТОЛЬКО подстановка `version="…"`: ns-набор/порядок/`xsi:type` дословно
/// совпадают у 2.20 (ERP 8.3.27, `Roles/**/Ext/Rights.xml`) и 2.21 (SSL 8.5.1, 107/107) —
/// сверено по обоим корпусам. Полный тег = PREFIX + `FormatVersion::to_string()` + SUFFIX.
const DESIGNER_RIGHTS_OPEN_PREFIX: &[u8] =
    b"<Rights xmlns=\"http://v8.1c.ru/8.2/roles\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"Rights\" version=\"";
/// Хвост открывающего тега Designer после значения `version=`.
const DESIGNER_RIGHTS_OPEN_SUFFIX: &[u8] = b"\">\r\n";

/// Имя конструкции «корневая обёртка Designer-прав» в таблице раскладок
/// (`models/format_layouts.jsonl`).
const RIGHTS_ENVELOPE_CONSTRUCT: &str = "designer.rights.envelope";

/// Витнессированные версии Designer-конверта прав — ОТВЕТ ДАННЫХ.
///
/// Список снят с реальных `Roles/**/Ext/Rights.xml` дампов одной конфигурации, собранных
/// каждой доступной платформой (`tools/build_format_layouts.py`): конверт прав версии не
/// несёт ничего, кроме значения `version=`, — ns-ростер и `xsi:type` во всех витнессированных
/// версиях совпадают ДОСЛОВНО.
///
/// Прочие версии НЕ экстраполируются (записи конверта прав действуют РОВНО в своей версии):
/// не-witnessed `version=` → отказ и на read, и на write (§1.0 — угаданный конверт молча дал
/// бы дамп, который платформа не поймёт).
fn designer_rights_versions() -> &'static [FormatVersion] {
    static VERSIONS: OnceLock<Vec<FormatVersion>> = OnceLock::new();
    VERSIONS.get_or_init(|| morph1c_core::version::layout::versions(RIGHTS_ENVELOPE_CONSTRUCT))
}

/// Собрать полный открывающий тег Designer `<Rights …>` витнессированной версии `v`.
fn designer_rights_open(v: FormatVersion) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        DESIGNER_RIGHTS_OPEN_PREFIX.len() + DESIGNER_RIGHTS_OPEN_SUFFIX.len() + 8,
    );
    out.extend_from_slice(DESIGNER_RIGHTS_OPEN_PREFIX);
    out.extend_from_slice(v.to_string().as_bytes());
    out.extend_from_slice(DESIGNER_RIGHTS_OPEN_SUFFIX);
    out
}

/// Человекочитаемый список витнессированных версий для сообщений об ошибке.
fn witnessed_rights_versions() -> String {
    designer_rights_versions()
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Детект версии Designer-конверта прав по байтам спутника (версионный вход РИДЕРА,
/// FORMATS.md §1: формат ВХОДА детектится по самому файлу). `None` — не Designer-конверт
/// прав ЛИБО не-witnessed версия (вызывающий решает, ошибка это или нет; [`read`] с
/// `DesignerRights` в обоих случаях отказывает громко).
pub fn detect_designer_rights_version(bytes: &[u8]) -> Option<FormatVersion> {
    let rest = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let lf_markup = rest.starts_with(structural_literal(XML_DECL, true).as_ref());
    let declaration = structural_literal(XML_DECL, lf_markup);
    let suffix = structural_literal(DESIGNER_RIGHTS_OPEN_SUFFIX, lf_markup);
    let rest = rest
        .strip_prefix(declaration.as_ref())?
        .strip_prefix(DESIGNER_RIGHTS_OPEN_PREFIX)?;
    let end = find(rest, suffix.as_ref())?;
    let vstr = std::str::from_utf8(&rest[..end]).ok()?;
    designer_rights_versions()
        .iter()
        .copied()
        .find(|v| v.to_string() == vstr)
}

/// Закрывающий тег корня.
const RIGHTS_CLOSE: &[u8] = b"</Rights>";
/// Финальный перевод строки после `</Rights>` (EDT — есть; Designer — нет).
const TRAILING_CRLF: &[u8] = b"\r\n";

impl SidecarFormat {
    /// Является ли формат RIGHTS-семейством (Role). XDTO-варианты (`*Xdto`) обслуживает
    /// `formats_xml::xdto`, а НЕ этот кодек: `read`/`write` их отвергают типизированной
    /// ошибкой ДО обращения к envelope-хелперам ниже (потому их `*Xdto`-арм недостижим).
    fn is_rights(self) -> bool {
        matches!(
            self,
            SidecarFormat::EdtRights | SidecarFormat::DesignerRights
        )
    }
    /// Есть ли у формата BOM в начале файла (Designer — да; EDT — нет).
    fn has_bom(self) -> bool {
        matches!(self, SidecarFormat::DesignerRights)
    }
    /// Есть ли финальный CRLF после `</Rights>` (EDT — да; Designer — нет).
    fn has_trailing_crlf(self) -> bool {
        matches!(self, SidecarFormat::EdtRights)
    }
}

// --- Перевод строки ВНУТРИ текста `<condition>` (RLS) ----------------------------------
//
// Диалекты хранят ВНУТРЕННИЕ переводы строк многострочного условия ПО-РАЗНОМУ (RE: SSL,
// Role `БазовыеПраваБСП` и ещё 32 из 107):
//   * EDT   `Rights.rights` — CRLF (6711 CRLF / 0 голых LF: и разметка, и текст условия);
//   * Designer `Ext/Rights.xml` — разметка CRLF, а текст условия ГОЛЫМ LF (3014 / 3696).
// Обе стороны в остальном байт-идентичны (`equal after CRLF→LF: true`).
//
// КАНОН = CRLF (конвенция EDT И cf): тело прав cf `<uuid>.0` пишется из этого же текста, и
// оракул `ssl.cf` совпадает с EDT байт-в-байт (гейт `role_rights_write_from_ir`, 107/107).
// Поэтому EDT читается/пишется ВЕРБАТИМ (cf-полоса не двигается), а транскодирует ТОЛЬКО
// Designer: read LF→CRLF, write CRLF→LF.
//
// До этого текст условия шёл ВЕРБАТИМ в обе стороны, из-за чего:
//   * designer→edt клал LF там, где EDT-источник несёт CRLF (33/107 ролей мимо байт-точности);
//   * edt→designer — зеркально (33/107 `Ext/Rights.xml`);
//   * designer→cf МОЛЧА писал LF-условия в тело прав, хотя оракул несёт CRLF — гейт
//     `role_rights_write_from_ir` этого не ловил (он гоняет ТОЛЬКО EDT-источник).

/// Нормализовать переводы строк текста: `to_crlf` → каждый `\n`/`\r\n` становится `\r\n`;
/// иначе каждый `\r\n` становится `\n`. Одиночный `\r` (не перевод строки) — данные, не
/// трогаем. Идемпотентна, поэтому смешанный текст не задваивает CR.
fn normalize_eol(text: &str, to_crlf: bool) -> String {
    let lf = text.replace("\r\n", "\n");
    if to_crlf {
        lf.replace('\n', "\r\n")
    } else {
        lf
    }
}

/// Текст `<condition>` из байтов спутника → КАНОН (CRLF). Designer хранит голые LF.
fn condition_to_canon(text: &str, fmt: SidecarFormat) -> String {
    match fmt {
        SidecarFormat::DesignerRights => normalize_eol(text, true),
        _ => text.to_string(), // EDT уже CRLF — вербатим (cf-полоса не меняется).
    }
}

/// КАНОН (CRLF) → текст `<condition>` в байтах спутника формата `fmt`.
fn condition_from_canon(text: &str, fmt: SidecarFormat) -> String {
    match fmt {
        SidecarFormat::DesignerRights => normalize_eol(text, false),
        _ => text.to_string(),
    }
}

/// Adapt only codec-authored XML literals, never text values or whole output.
fn structural_literal(literal: &[u8], lf_markup: bool) -> Cow<'_, [u8]> {
    if !lf_markup {
        return Cow::Borrowed(literal);
    }
    let mut out = Vec::with_capacity(literal.len());
    let mut index = 0;
    while index < literal.len() {
        if literal[index] == b'\r' && literal.get(index + 1) == Some(&b'\n') {
            index += 1;
        }
        out.push(literal[index]);
        index += 1;
    }
    Cow::Owned(out)
}

/// The source layout is skipped by serde. Hash the entire current semantic
/// table through streaming serialization, without cloning any conditions.
fn canonical_digest(table: &RightsTable) -> Result<[u8; 32], RightsError> {
    struct HashWriter(Sha256);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = std::io::BufWriter::with_capacity(64 * 1024, HashWriter(Sha256::new()));
    serde_json::to_writer(&mut writer, table)
        .map_err(|error| RightsError(format!("canonical rights serialization: {error}")))?;
    let writer = writer
        .into_inner()
        .map_err(|error| RightsError(format!("canonical rights serialization flush: {error}")))?;
    Ok(writer.0.finalize().into())
}

fn newline_pattern(text: &str) -> Vec<bool> {
    let bytes = text.as_bytes();
    bytes
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| {
            (*byte == b'\n').then(|| index > 0 && bytes[index - 1] == b'\r')
        })
        .collect()
}

fn conditions(table: &RightsTable) -> impl Iterator<Item = &str> {
    table
        .objects
        .iter()
        .flat_map(|object| object.rights.iter())
        .flat_map(|right| right.restrictions.iter())
        .map(|restriction| restriction.condition.as_str())
        .chain(
            table
                .restriction_templates
                .iter()
                .map(|template| template.condition.as_str()),
        )
}

fn restore_condition(text: &str, pattern: &[bool]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newline = 0;
    for (index, piece) in text.split('\n').enumerate() {
        if index != 0 {
            if pattern[newline] {
                out.push('\r');
            }
            out.push('\n');
            newline += 1;
        }
        // A CR immediately before LF belongs to that newline. Standalone CR
        // elsewhere, including the last piece, remains current text data.
        let piece = if index < pattern.len() {
            piece.strip_suffix('\r').unwrap_or(piece)
        } else {
            piece
        };
        out.push_str(piece);
    }
    out
}

// --- READ: байты спутника → RightsTable -----------------------------------------------

/// Курсор строгого байтового спуска по телу спутника: потребляет ТОЧНЫЕ литералы, иначе
/// типизированная ошибка (§1.0). Держит смещение для точной диагностики и ФОРМАТ спутника —
/// он нужен листовым читателям для канонизации EOL текста `<condition>` (см.
/// [`condition_to_canon`]).
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    fmt: SidecarFormat,
    lf_markup: bool,
    condition_newlines: Vec<Vec<bool>>,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], fmt: SidecarFormat) -> Self {
        let rest = data.strip_prefix(BOM).unwrap_or(data);
        let lf_markup = rest.starts_with(structural_literal(XML_DECL, true).as_ref());
        Cursor {
            data,
            pos: 0,
            fmt,
            lf_markup,
            condition_newlines: Vec::new(),
        }
    }

    /// Потребить точный литерал `lit` или ошибка с контекстом.
    fn expect(&mut self, lit: &[u8], ctx: &str) -> Result<(), RightsError> {
        let lit = structural_literal(lit, self.lf_markup);
        if self.data[self.pos..].starts_with(lit.as_ref()) {
            self.pos += lit.len();
            Ok(())
        } else {
            let got = &self.data[self.pos..(self.pos + lit.len().min(48)).min(self.data.len())];
            err(format!(
                "at offset {}: expected {ctx} {:?}, found {:?}",
                self.pos,
                String::from_utf8_lossy(lit.as_ref()),
                String::from_utf8_lossy(got)
            ))
        }
    }

    /// Взгляд: начинается ли остаток с `lit` (без потребления).
    fn peek(&self, lit: &[u8]) -> bool {
        self.data[self.pos..].starts_with(structural_literal(lit, self.lf_markup).as_ref())
    }

    /// Прочитать текст до `close` (не включая), потребить `close`. Текст НЕ должен нести
    /// `<` (иначе это вложенный тег, а не лист-текст → ошибка §1.0). Возвращает текст.
    fn take_text_until(&mut self, close: &[u8], ctx: &str) -> Result<&'a str, RightsError> {
        let close = structural_literal(close, self.lf_markup);
        let rest = &self.data[self.pos..];
        let end = find(rest, close.as_ref()).ok_or_else(|| {
            RightsError(format!(
                "at offset {}: unterminated {ctx} (no {:?})",
                self.pos,
                String::from_utf8_lossy(close.as_ref())
            ))
        })?;
        let text_bytes = &rest[..end];
        if text_bytes.contains(&b'<') {
            return err(format!(
                "at offset {}: unexpected '<' in {ctx} text",
                self.pos
            ));
        }
        let text = std::str::from_utf8(text_bytes)
            .map_err(|e| RightsError(format!("at offset {}: non-utf8 {ctx}: {e}", self.pos)))?;
        self.pos += end + close.len();
        Ok(text)
    }

    fn condition(&mut self, close: &[u8], ctx: &str) -> Result<String, RightsError> {
        let text = self.take_text_until(close, ctx)?;
        self.condition_newlines.push(newline_pattern(text));
        Ok(condition_to_canon(text, self.fmt))
    }

    /// Остались ли ещё непрочитанные байты.
    fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }
}

/// Найти первое вхождение `needle` в `hay`.
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Разобрать текст флага/значения в bool (`true`/`false`) или ошибка (§1.0 — не догадка).
fn parse_bool(text: &str, ctx: &str) -> Result<bool, RightsError> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        other => err(format!(
            "{ctx}: expected boolean 'true'/'false', got {other:?}"
        )),
    }
}

/// Прочитать байты спутника (`Rights.rights`/`Ext/Rights.xml`) в [`RightsTable`],
/// СВЕРЯЯ envelope формата `fmt` byte-exact. §1.0: любое отклонение формы → ошибка.
pub fn read(bytes: &[u8], fmt: SidecarFormat) -> Result<RightsTable, RightsError> {
    if !fmt.is_rights() {
        return err(format!(
            "rights::read called with non-Rights sidecar format {fmt:?}"
        ));
    }
    let mut c = Cursor::new(bytes, fmt);

    // --- envelope: BOM (по формату) + XML-декларация + открывающий <Rights …> ---
    let bom = c.peek(BOM);
    if fmt.has_bom() && bom {
        c.expect(BOM, "BOM")?;
    } else if !fmt.has_bom() && bom {
        return err("EDT rights sidecar must not carry a BOM");
    }
    c.expect(XML_DECL, "XML declaration")?;
    match fmt {
        SidecarFormat::EdtRights => c.expect(EDT_RIGHTS_OPEN, "<Rights> open tag")?,
        SidecarFormat::DesignerRights => {
            // Версионный вход РИДЕРА (FORMATS.md §1): конверт 2.20/2.21 дословно совпадает
            // во всём, КРОМЕ значения `version=` — детектим его ПО САМОМУ файлу и принимаем
            // ТОЛЬКО витнессированные версии (§1.0: не-witnessed → отказ, не догадка).
            c.expect(DESIGNER_RIGHTS_OPEN_PREFIX, "<Rights> open tag")?;
            let vstr =
                c.take_text_until(DESIGNER_RIGHTS_OPEN_SUFFIX, "<Rights> version attribute")?;
            if !designer_rights_versions()
                .iter()
                .any(|v| v.to_string() == vstr)
            {
                return err(format!(
                    "unrecognized Designer <Rights> format version {vstr:?} \
                     (witnessed: {})",
                    witnessed_rights_versions()
                ));
            }
        }
        // Guard `is_rights` выше делает XDTO-арм недостижимым.
        SidecarFormat::EdtXdto | SidecarFormat::DesignerXdto => {
            unreachable!("rights::read open tag on XDTO format — guarded by is_rights")
        }
    }

    // --- три флага корня в ФИКСИРОВАННОМ порядке (сверено 107/107) ---
    let set_for_new_objects = read_flag(&mut c, "setForNewObjects")?;
    let set_for_attributes_by_default = read_flag(&mut c, "setForAttributesByDefault")?;
    let independent_rights_of_child_objects = read_flag(&mut c, "independentRightsOfChildObjects")?;

    // --- список <object> (0..n) ---
    let mut objects = Vec::new();
    while c.peek(b"\t<object>\r\n") {
        objects.push(read_object(&mut c)?);
    }

    // --- список <restrictionTemplate> (0..n) — top-level ПОСЛЕ объектов ---
    let mut restriction_templates = Vec::new();
    while c.peek(b"\t<restrictionTemplate>\r\n") {
        restriction_templates.push(read_restriction_template(&mut c)?);
    }

    // --- закрытие корня + трейлер по формату ---
    c.expect(RIGHTS_CLOSE, "</Rights>")?;
    let trailing_newline = c.peek(TRAILING_CRLF);
    if fmt.has_trailing_crlf() || trailing_newline {
        c.expect(TRAILING_CRLF, "trailing newline")?;
    }
    if !c.at_end() {
        return err(format!(
            "at offset {}: trailing bytes after </Rights>",
            c.pos
        ));
    }

    let mut table = RightsTable {
        set_for_new_objects,
        set_for_attributes_by_default,
        independent_rights_of_child_objects,
        objects,
        restriction_templates,
        source_layout: None,
    };
    table.source_layout = Some(RightsSourceLayout {
        designer: fmt == SidecarFormat::DesignerRights,
        bom,
        lf_markup: c.lf_markup,
        trailing_newline,
        canonical_sha256: canonical_digest(&table)?,
        condition_newlines: c.condition_newlines,
    });
    Ok(table)
}

/// Прочитать один флаг `\t<tag>bool</tag>\r\n`.
fn read_flag(c: &mut Cursor, tag: &str) -> Result<bool, RightsError> {
    c.expect(format!("\t<{tag}>").as_bytes(), &format!("<{tag}>"))?;
    let text = c.take_text_until(
        format!("</{tag}>\r\n").as_bytes(),
        &format!("<{tag}> value"),
    )?;
    parse_bool(text, tag)
}

/// Прочитать один `<object>` блок (name + rights).
fn read_object(c: &mut Cursor) -> Result<RightsObject, RightsError> {
    c.expect(b"\t<object>\r\n", "<object>")?;
    c.expect(b"\t\t<name>", "<object><name>")?;
    let name = c
        .take_text_until(b"</name>\r\n", "object name")?
        .to_string();

    let mut rights = Vec::new();
    while c.peek(b"\t\t<right>\r\n") {
        rights.push(read_right(c)?);
    }

    c.expect(b"\t</object>\r\n", "</object>")?;
    Ok(RightsObject { name, rights })
}

/// Прочитать один `<right>` блок со всеми ограничениями в исходном порядке.
fn read_right(c: &mut Cursor) -> Result<Right, RightsError> {
    c.expect(b"\t\t<right>\r\n", "<right>")?;
    c.expect(b"\t\t\t<name>", "<right><name>")?;
    let name = c.take_text_until(b"</name>\r\n", "right name")?.to_string();
    c.expect(b"\t\t\t<value>", "<right><value>")?;
    let value_text = c.take_text_until(b"</value>\r\n", "right value")?;
    let value = parse_bool(value_text, "right value")?;

    let mut restrictions = Vec::new();
    while c.peek(b"\t\t\t<restrictionByCondition>\r\n") {
        restrictions.push(read_restriction(c)?);
    }

    c.expect(b"\t\t</right>\r\n", "</right>")?;
    Ok(Right {
        name,
        value,
        restrictions,
    })
}

/// Прочитать `<restrictionByCondition>`: опц. `<field>` + `<condition>` (текст ВЕРБАТИМ).
fn read_restriction(c: &mut Cursor) -> Result<RightRestriction, RightsError> {
    c.expect(
        b"\t\t\t<restrictionByCondition>\r\n",
        "<restrictionByCondition>",
    )?;
    // Опц. <field> ПЕРЕД <condition> (1 вхождение в SSL).
    let field = if c.peek(b"\t\t\t\t<field>") {
        c.expect(b"\t\t\t\t<field>", "<field>")?;
        Some(c.take_text_until(b"</field>\r\n", "field")?.to_string())
    } else {
        None
    };
    c.expect(b"\t\t\t\t<condition>", "<condition>")?;
    // Многострочный BSL/SQL (с &amp;/&gt;/&lt;; без литерального '<'). EOL — к КАНОНУ (CRLF):
    // Designer хранит внутренние переводы строк голым LF (см. `condition_to_canon`).
    let condition = c.condition(b"</condition>\r\n", "condition")?;
    c.expect(
        b"\t\t\t</restrictionByCondition>\r\n",
        "</restrictionByCondition>",
    )?;
    Ok(RightRestriction { field, condition })
}

/// Прочитать top-level `<restrictionTemplate>`: `<name>` + `<condition>` (текст ВЕРБАТИМ).
fn read_restriction_template(c: &mut Cursor) -> Result<RestrictionTemplate, RightsError> {
    c.expect(b"\t<restrictionTemplate>\r\n", "<restrictionTemplate>")?;
    c.expect(b"\t\t<name>", "<restrictionTemplate><name>")?;
    let name = c
        .take_text_until(b"</name>\r\n", "template name")?
        .to_string();
    c.expect(b"\t\t<condition>", "<restrictionTemplate><condition>")?;
    let condition = c.condition(b"</condition>\r\n", "template condition")?;
    c.expect(b"\t</restrictionTemplate>\r\n", "</restrictionTemplate>")?;
    Ok(RestrictionTemplate { name, condition })
}

// --- WRITE: RightsTable → байты спутника (byte-exact) ---------------------------------

struct RightsOutput {
    bytes: Vec<u8>,
    lf_markup: bool,
}
impl RightsOutput {
    fn literal(&mut self, literal: &[u8]) {
        self.bytes
            .extend_from_slice(structural_literal(literal, self.lf_markup).as_ref());
    }
    fn value(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }
}

/// Регенерировать байты спутника из [`RightsTable`] под envelope формата `fmt` (byte-exact
/// с источником, если IR получен `read(…, fmt)` при том же таргете). НЕ эхо входа —
/// строится из IR+envelope.
///
/// # Версия ПИСАТЕЛЯ (FORMATS.md §1: формат выхода — параметр)
/// Designer-конверт несёт `version="…"` — значение берётся из амбьентного round-trip
/// таргета ([`morph1c_core::version::with_roundtrip_target`]; вне scope — SSL 2.21, прежнее
/// поведение). Не-witnessed таргет → типизированная ошибка (§1.0: не молчаливый дефолт;
/// зеркало `formats_designer::common::emit_root_envelope`). EDT-конверт версии не несёт.
pub fn write(table: &RightsTable, fmt: SidecarFormat) -> Result<Vec<u8>, RightsError> {
    if !fmt.is_rights() {
        return err(format!(
            "rights::write called with non-Rights sidecar format {fmt:?}"
        ));
    }
    let source = table
        .source_layout
        .as_ref()
        .filter(|layout| layout.designer == (fmt == SidecarFormat::DesignerRights));
    let lf_markup = source.is_some_and(|layout| layout.lf_markup);
    let bom = source.map_or(fmt.has_bom(), |layout| layout.bom);
    let trailing_newline = source.map_or(fmt.has_trailing_crlf(), |layout| layout.trailing_newline);
    let pattern_layout = source.filter(|layout| {
        fmt == SidecarFormat::DesignerRights
            && layout.condition_newlines.len() == conditions(table).count()
            && conditions(table)
                .zip(&layout.condition_newlines)
                .all(|(text, pattern)| {
                    text.bytes().filter(|byte| *byte == b'\n').count() == pattern.len()
                })
    });
    let pattern_layout = match pattern_layout {
        Some(layout) if layout.canonical_sha256 == canonical_digest(table)? => Some(layout),
        _ => None,
    };
    let mut condition_index = 0;
    let mut out = RightsOutput {
        bytes: Vec::new(),
        lf_markup,
    };
    if bom {
        out.literal(BOM);
    }
    out.literal(XML_DECL);
    match fmt {
        SidecarFormat::EdtRights => out.literal(EDT_RIGHTS_OPEN),
        SidecarFormat::DesignerRights => {
            let target = morph1c_core::version::current_roundtrip_target().unwrap_or(SSL);
            if !designer_rights_versions().contains(&target) {
                return err(format!(
                    "no witnessed Designer <Rights> envelope for format {target} \
                     (witnessed: {})",
                    witnessed_rights_versions()
                ));
            }
            out.literal(&designer_rights_open(target));
        }
        // Guard `is_rights` выше делает XDTO-арм недостижимым.
        SidecarFormat::EdtXdto | SidecarFormat::DesignerXdto => {
            unreachable!("rights::write open tag on XDTO format — guarded by is_rights")
        }
    }

    write_flag(&mut out, "setForNewObjects", table.set_for_new_objects);
    write_flag(
        &mut out,
        "setForAttributesByDefault",
        table.set_for_attributes_by_default,
    );
    write_flag(
        &mut out,
        "independentRightsOfChildObjects",
        table.independent_rights_of_child_objects,
    );

    for obj in &table.objects {
        out.literal(b"\t<object>\r\n\t\t<name>");
        out.value(obj.name.as_bytes());
        out.literal(b"</name>\r\n");
        for right in &obj.rights {
            out.literal(b"\t\t<right>\r\n\t\t\t<name>");
            out.value(right.name.as_bytes());
            out.literal(b"</name>\r\n\t\t\t<value>");
            out.value(bool_str(right.value).as_bytes());
            out.literal(b"</value>\r\n");
            for r in &right.restrictions {
                out.literal(b"\t\t\t<restrictionByCondition>\r\n");
                if let Some(field) = &r.field {
                    out.literal(b"\t\t\t\t<field>");
                    out.value(field.as_bytes());
                    out.literal(b"</field>\r\n");
                }
                out.literal(b"\t\t\t\t<condition>");
                let condition = pattern_layout.map_or_else(
                    || condition_from_canon(&r.condition, fmt),
                    |layout| {
                        restore_condition(&r.condition, &layout.condition_newlines[condition_index])
                    },
                );
                condition_index += 1;
                out.value(condition.as_bytes());
                out.literal(b"</condition>\r\n\t\t\t</restrictionByCondition>\r\n");
            }
            out.literal(b"\t\t</right>\r\n");
        }
        out.literal(b"\t</object>\r\n");
    }

    // <restrictionTemplate> — top-level, ПОСЛЕ всех объектов.
    for tpl in &table.restriction_templates {
        out.literal(b"\t<restrictionTemplate>\r\n\t\t<name>");
        out.value(tpl.name.as_bytes());
        out.literal(b"</name>\r\n\t\t<condition>");
        let condition = pattern_layout.map_or_else(
            || condition_from_canon(&tpl.condition, fmt),
            |layout| restore_condition(&tpl.condition, &layout.condition_newlines[condition_index]),
        );
        condition_index += 1;
        out.value(condition.as_bytes());
        out.literal(b"</condition>\r\n\t</restrictionTemplate>\r\n");
    }

    out.literal(RIGHTS_CLOSE);
    if trailing_newline {
        out.literal(TRAILING_CRLF);
    }
    Ok(out.bytes)
}

fn write_flag(out: &mut RightsOutput, tag: &str, value: bool) {
    out.literal(format!("\t<{tag}>{}</{tag}>\r\n", bool_str(value)).as_bytes());
}

fn bool_str(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::version::ERP;

    /// Минимальный EDT-спутник: 2 флага true/false + один объект с двумя правами.
    fn edt_sample() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(XML_DECL);
        v.extend_from_slice(EDT_RIGHTS_OPEN);
        v.extend_from_slice(b"\t<setForNewObjects>false</setForNewObjects>\r\n");
        v.extend_from_slice(b"\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n");
        v.extend_from_slice(
            b"\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n",
        );
        v.extend_from_slice(b"\t<object>\r\n\t\t<name>Configuration.X</name>\r\n");
        v.extend_from_slice(b"\t\t<right>\r\n\t\t\t<name>Read</name>\r\n\t\t\t<value>true</value>\r\n\t\t</right>\r\n");
        v.extend_from_slice(b"\t\t<right>\r\n\t\t\t<name>Update</name>\r\n\t\t\t<value>false</value>\r\n\t\t</right>\r\n");
        v.extend_from_slice(b"\t</object>\r\n");
        v.extend_from_slice(RIGHTS_CLOSE);
        v.extend_from_slice(TRAILING_CRLF);
        v
    }

    #[test]
    fn edt_roundtrips_byte_exact() {
        let src = edt_sample();
        let table = read(&src, SidecarFormat::EdtRights).unwrap();
        assert_eq!(table.objects.len(), 1);
        assert_eq!(table.objects[0].name, "Configuration.X");
        assert_eq!(table.objects[0].rights.len(), 2);
        assert_eq!(table.objects[0].rights[0].name, "Read");
        assert!(table.objects[0].rights[0].value);
        assert_eq!(table.objects[0].rights[1].name, "Update");
        assert!(!table.objects[0].rights[1].value);
        assert!(!table.set_for_new_objects);
        assert!(table.set_for_attributes_by_default);
        assert_eq!(
            write(&table, SidecarFormat::EdtRights).unwrap(),
            src,
            "EDT byte-exact round-trip"
        );
    }

    /// EDT-спутник с `<restrictionByCondition>` (field + multi-line condition) +
    /// top-level `<restrictionTemplate>` round-trips byte-exact.
    fn edt_sample_with_restriction() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(XML_DECL);
        v.extend_from_slice(EDT_RIGHTS_OPEN);
        v.extend_from_slice(b"\t<setForNewObjects>false</setForNewObjects>\r\n");
        v.extend_from_slice(b"\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n");
        v.extend_from_slice(
            b"\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n",
        );
        v.extend_from_slice(b"\t<object>\r\n\t\t<name>Catalog.Y</name>\r\n");
        v.extend_from_slice(
            b"\t\t<right>\r\n\t\t\t<name>Read</name>\r\n\t\t\t<value>true</value>\r\n",
        );
        v.extend_from_slice(b"\t\t\t<restrictionByCondition>\r\n\t\t\t\t<field>\xd0\xa1\xd1\x81\xd1\x8b\xd0\xbb\xd0\xba\xd0\xb0</field>\r\n");
        // multi-line condition text with &amp; entity and embedded \r\n.
        v.extend_from_slice(b"\t\t\t\t<condition>#\xd0\x95\xd1\x81\xd0\xbb\xd0\xb8 &amp;X #\xd0\xa2\xd0\xbe\xd0\xb3\xd0\xb4\xd0\xb0\r\n\xd0\x93\xd0\x94\xd0\x95 \xd0\x9b\xd0\x9e\xd0\x96\xd0\xac</condition>\r\n");
        v.extend_from_slice(b"\t\t\t</restrictionByCondition>\r\n\t\t</right>\r\n");
        v.extend_from_slice(b"\t</object>\r\n");
        v.extend_from_slice(b"\t<restrictionTemplate>\r\n\t\t<name>\xd0\x94\xd0\xbb\xd1\x8f\xd0\x9e\xd0\xb1\xd1\x8a\xd0\xb5\xd0\xba\xd1\x82\xd0\xb0</name>\r\n");
        v.extend_from_slice(b"\t\t<condition>// tmpl\r\n\xd0\x93\xd0\x94\xd0\x95 \xd0\x98\xd0\xa1\xd0\xa2\xd0\x98\xd0\x9d\xd0\x90</condition>\r\n\t</restrictionTemplate>\r\n");
        v.extend_from_slice(RIGHTS_CLOSE);
        v.extend_from_slice(TRAILING_CRLF);
        v
    }

    #[test]
    fn edt_restriction_and_template_roundtrip_byte_exact() {
        let src = edt_sample_with_restriction();
        let table = read(&src, SidecarFormat::EdtRights).unwrap();
        assert_eq!(table.objects.len(), 1);
        let right = &table.objects[0].rights[0];
        let restr = right.restrictions.first().expect("has restriction");
        assert_eq!(restr.field.as_deref(), Some("Ссылка"));
        assert!(restr.condition.contains("&amp;X"));
        assert!(
            restr.condition.contains("\r\n"),
            "multi-line condition captured verbatim"
        );
        assert_eq!(table.restriction_templates.len(), 1);
        assert_eq!(table.restriction_templates[0].name, "ДляОбъекта");
        assert_eq!(
            write(&table, SidecarFormat::EdtRights).unwrap(),
            src,
            "restriction+template byte-exact"
        );
    }

    /// Designer-спутник версии `v` с фиксированным телом-тремя-флагами (для версионных тестов).
    fn designer_sample(v: FormatVersion) -> Vec<u8> {
        let mut src = Vec::new();
        src.extend_from_slice(BOM);
        src.extend_from_slice(XML_DECL);
        src.extend_from_slice(&designer_rights_open(v));
        src.extend_from_slice(b"\t<setForNewObjects>false</setForNewObjects>\r\n");
        src.extend_from_slice(b"\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n");
        src.extend_from_slice(
            b"\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n",
        );
        src.extend_from_slice(RIGHTS_CLOSE);
        src
    }

    #[test]
    fn designer_envelope_differs_but_body_equal() {
        // Same body, Designer envelope (BOM + version + no trailing newline). БЕЗ амбьентного
        // таргета write обязан выдавать ПРЕЖНИЙ 2.21-конверт (SSL-путь байт-в-байт).
        let src = designer_sample(SSL);
        assert!(
            find(&src, b" version=\"2.21\">\r\n").is_some(),
            "SSL sample must carry the literal 2.21 envelope"
        );
        let table = read(&src, SidecarFormat::DesignerRights).unwrap();
        assert!(table.objects.is_empty());
        assert_eq!(
            write(&table, SidecarFormat::DesignerRights).unwrap(),
            src,
            "Designer byte-exact round-trip"
        );

        // Cross-format X: EDT body with same flags/objects parses to the SAME RightsTable.
        let mut edt = Vec::new();
        edt.extend_from_slice(XML_DECL);
        edt.extend_from_slice(EDT_RIGHTS_OPEN);
        edt.extend_from_slice(b"\t<setForNewObjects>false</setForNewObjects>\r\n");
        edt.extend_from_slice(b"\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n");
        edt.extend_from_slice(
            b"\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n",
        );
        edt.extend_from_slice(RIGHTS_CLOSE);
        edt.extend_from_slice(TRAILING_CRLF);
        let edt_table = read(&edt, SidecarFormat::EdtRights).unwrap();
        assert_eq!(
            edt_table, table,
            "EDT and Designer parse identical body to equal RightsTable (X)"
        );
    }

    #[test]
    fn rejects_bom_on_edt() {
        let mut src = Vec::from(BOM);
        src.extend_from_slice(&edt_sample());
        assert!(
            read(&src, SidecarFormat::EdtRights).is_err(),
            "EDT must reject a BOM (§1.0)"
        );
    }

    #[test]
    fn rejects_unknown_flag_value() {
        let mut src = Vec::new();
        src.extend_from_slice(XML_DECL);
        src.extend_from_slice(EDT_RIGHTS_OPEN);
        src.extend_from_slice(b"\t<setForNewObjects>maybe</setForNewObjects>\r\n");
        assert!(
            read(&src, SidecarFormat::EdtRights).is_err(),
            "non-bool flag → error (§1.0)"
        );
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut src = edt_sample();
        src.extend_from_slice(b"garbage");
        assert!(
            read(&src, SidecarFormat::EdtRights).is_err(),
            "trailing bytes → error (§1.0)"
        );
    }

    #[test]
    fn rejects_wrong_namespace() {
        let mut src = Vec::new();
        src.extend_from_slice(XML_DECL);
        src.extend_from_slice(b"<Rights xmlns=\"http://wrong/ns\">\r\n");
        assert!(
            read(&src, SidecarFormat::EdtRights).is_err(),
            "wrong ns → envelope error (§1.0)"
        );
    }

    /// EOL внутри `<condition>`: EDT хранит CRLF, Designer — голый LF (RE: SSL, 33/107 ролей).
    /// Канон = CRLF (конвенция EDT И тела прав cf) → EDT вербатим, Designer транскодирует.
    #[test]
    fn condition_eol_is_canonical_crlf_and_designer_transcodes() {
        assert_eq!(normalize_eol("a\nb\r\nc", true), "a\r\nb\r\nc");
        assert_eq!(normalize_eol("a\nb\r\nc", false), "a\nb\nc");
        // Идемпотентна — смешанный текст не задваивает CR.
        assert_eq!(normalize_eol(&normalize_eol("a\nb", true), true), "a\r\nb");

        // Designer: голый LF на диске ↔ CRLF в каноне.
        assert_eq!(
            condition_to_canon("ГДЕ X\nИЛИ Y", SidecarFormat::DesignerRights),
            "ГДЕ X\r\nИЛИ Y"
        );
        assert_eq!(
            condition_from_canon("ГДЕ X\r\nИЛИ Y", SidecarFormat::DesignerRights),
            "ГДЕ X\nИЛИ Y"
        );
        // EDT: ВЕРБАТИМ в обе стороны (cf-полоса пишется из этого же текста и совпадает с
        // оракулом байт-в-байт — транскодировать её нельзя).
        assert_eq!(
            condition_to_canon("ГДЕ X\r\nИЛИ Y", SidecarFormat::EdtRights),
            "ГДЕ X\r\nИЛИ Y"
        );
        assert_eq!(
            condition_from_canon("ГДЕ X\r\nИЛИ Y", SidecarFormat::EdtRights),
            "ГДЕ X\r\nИЛИ Y"
        );
    }

    /// §1.0-самопроверка: read→write в СВОЁМ диалекте воспроизводит исходные байты, а канон
    /// (CRLF) один и тот же у обоих диалектов — то есть edt↔designer байт-точен в обе стороны.
    #[test]
    fn multiline_condition_round_trips_byte_exact_in_both_dialects() {
        let edt = edt_sample_with_restriction();
        let t_edt = read(&edt, SidecarFormat::EdtRights).expect("edt read");
        assert_eq!(
            write(&t_edt, SidecarFormat::EdtRights).unwrap(),
            edt,
            "EDT read→write byte-exact"
        );

        // Тот же канон → Designer-байты, и обратно.
        let des = write(&t_edt, SidecarFormat::DesignerRights).unwrap();
        let t_des = read(&des, SidecarFormat::DesignerRights).expect("designer read");
        assert_eq!(
            t_des, t_edt,
            "ОДИН канон из обоих диалектов (§1.6) — условие с CRLF"
        );
        assert_eq!(
            write(&t_des, SidecarFormat::DesignerRights).unwrap(),
            des,
            "Designer read→write byte-exact"
        );
        assert_eq!(
            write(&t_des, SidecarFormat::EdtRights).unwrap(),
            edt,
            "designer→edt воспроизводит EDT-байты (CRLF в условии)"
        );

        // На диске Designer несёт ГОЛЫЕ LF внутри условия (а разметка — CRLF).
        let inner = {
            let s = String::from_utf8(des.clone()).unwrap();
            let i = s.find("<condition>").unwrap() + "<condition>".len();
            let j = s.find("</condition>").unwrap();
            s[i..j].to_string()
        };
        assert!(
            inner.contains('\n') && !inner.contains("\r\n"),
            "Designer condition uses bare LF on disk, got {inner:?}"
        );
    }

    /// ERP-конверт (2.20): read детектит версию ПО ФАЙЛУ, write таргет-версией воспроизводит
    /// байт-в-байт; тот же 2.20-файл даёт РАВНУЮ таблицу с 2.21-конвертом (version ≠ IR).
    #[test]
    fn designer_2_20_reads_and_writes_byte_exact_under_erp_target() {
        let src = designer_sample(ERP);
        assert!(
            find(&src, b" version=\"2.20\">\r\n").is_some(),
            "ERP sample must carry the literal 2.20 envelope"
        );
        let table = read(&src, SidecarFormat::DesignerRights).expect("2.20 must parse");
        // Версия — свойство ФАЙЛА, не IR: тот же корпус под 2.21-конвертом даёт РАВНУЮ таблицу.
        let ssl_table = read(&designer_sample(SSL), SidecarFormat::DesignerRights).unwrap();
        assert_eq!(table, ssl_table, "version must not leak into RightsTable");
        // Write ТАРГЕТ-версией (амбьентный round-trip таргет) → byte-exact.
        let regen = morph1c_core::version::with_roundtrip_target(ERP, || {
            write(&table, SidecarFormat::DesignerRights)
        })
        .unwrap();
        assert_eq!(regen, src, "2.20 Designer byte-exact round-trip");
        // Детект-хелпер видит ту же версию.
        assert_eq!(detect_designer_rights_version(&src), Some(ERP));
        assert_eq!(
            detect_designer_rights_version(&designer_sample(SSL)),
            Some(SSL)
        );
    }

    /// §1.0: не-witnessed версия — отказ и на read (чужой `version=`), и на write
    /// (не-witnessed амбьентный таргет), а НЕ молчаливый дефолт.
    #[test]
    fn designer_unwitnessed_version_is_refused_on_read_and_write() {
        let src = designer_sample(FormatVersion::new(2, 19));
        let e = read(&src, SidecarFormat::DesignerRights)
            .expect_err("2.19 envelope is not witnessed — read must refuse");
        assert!(
            e.to_string().contains("2.19") && e.to_string().contains("witnessed"),
            "error must name the version and the witnessed set, got: {e}"
        );
        assert_eq!(detect_designer_rights_version(&src), None);

        let table = read(&designer_sample(SSL), SidecarFormat::DesignerRights).unwrap();
        let e = morph1c_core::version::with_roundtrip_target(FormatVersion::new(2, 19), || {
            write(&table, SidecarFormat::DesignerRights)
        })
        .expect_err("2.19 target has no witnessed rights envelope — write must refuse");
        assert!(
            e.to_string().contains("2.19"),
            "write error must name the target, got: {e}"
        );
    }

    /// 2.17-конверт прав ВИТНЕССИРОВАН (`scripts/version_probe.sh`: `Roles/**/Ext/Rights.xml`
    /// 2.17 и 2.20 байт-идентичны после подстановки версии) ⇒ и read, и write под 2.17 обязаны
    /// работать: без этого 2.17-источник не читался вовсе.
    #[test]
    fn designer_2_17_rights_envelope_round_trips() {
        const V217: FormatVersion = FormatVersion::new(2, 17);
        let src = designer_sample(V217);
        assert_eq!(detect_designer_rights_version(&src), Some(V217));
        let table = read(&src, SidecarFormat::DesignerRights).expect("2.17 must parse");
        let regen = morph1c_core::version::with_roundtrip_target(V217, || {
            write(&table, SidecarFormat::DesignerRights)
        })
        .expect("2.17 is a witnessed rights envelope");
        assert_eq!(regen, src, "2.17 Designer rights byte-exact round-trip");
    }
}
