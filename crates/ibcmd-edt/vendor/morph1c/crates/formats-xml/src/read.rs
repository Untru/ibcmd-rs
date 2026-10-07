//! Токенизация XML-дескриптора в структурное дерево [`Element`] (§1.0-граница).
//!
//! Используем `quick-xml` ТОЛЬКО как токенайзер: события Start/Empty/End/Text/Decl
//! материализуем в [`Descriptor`]. Это даёт честный учёт тотальности (мы строим
//! дерево из РЕАЛЬНЫХ событий, а не угадываем структуру), а byte-exact восстановление
//! — отдельный структурный эмиттер (`emit.rs`).
//!
//! Тут НЕТ форматной семантики (имён полей): это общий XML-слой. Сверку envelope
//! (какой root/ns ожидается) делает коннектор формата поверх.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::descriptor::{Attr, Element};

/// Стиль перевода строки, наблюдённый в СЫРЫХ байтах документа (M2). Захватывается
/// до токенизации, чтобы коннектор сверил его с envelope формата (а не «молча
/// исправил» на запись).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolStyle {
    /// `\r\n` во всех переводах строки (EDT/Designer).
    Crlf,
    /// `\n` (без `\r`) хотя бы в одном переводе строки.
    Lf,
    /// Переводов строки нет вовсе (одна строка) — не противоречит ни одному формату.
    None,
}

/// Байтовая обёртка документа, наблюдённая на чтении (M2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteEnvelope {
    /// Присутствует ли UTF-8 BOM (`EF BB BF`) в начале.
    pub bom: bool,
    /// Стиль EOL.
    pub eol: EolStyle,
}

/// Разобранный дескриптор: байтовая обёртка + пролог + корневой элемент.
#[derive(Debug, Clone)]
pub struct Descriptor {
    /// Наблюдённая байтовая обёртка (BOM/EOL) — для сверки envelope (M2).
    pub bytes_env: ByteEnvelope,
    /// Точные байты XML-декларации, БЕЗ завершающего EOL (напр.
    /// `<?xml version="1.0" encoding="UTF-8"?>`). `None` = декларации не было.
    pub decl: Option<String>,
    /// Корневой элемент объекта.
    pub root: Element,
}

/// Определить байтовую обёртку (BOM + EOL-стиль) из сырых байтов документа.
///
/// СТРУКТУРНЫЙ EOL = тот, что разделяет узлы. Бар-LF ВНУТРИ ТЕКСТА (напр. многострочное
/// `<v8:content>` пояснения) — это ДАННЫЕ, а не envelope: они переживают round-trip
/// дословно (эмиттер не трогает `\n` в тексте). Поэтому: документ с ХОТЯ БЫ ОДНИМ CRLF
/// классифицируется как `Crlf` (его структура — CRLF), даже если в тексте есть bare LF.
/// `Lf` — только если CRLF НЕТ вовсе (структура — bare LF). Это не ослабляет R: byte-
/// exact round-trip остаётся гарантией (текстовый `\n` воспроизводится как есть).
fn observe_envelope(bytes: &[u8]) -> ByteEnvelope {
    let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let mut has_nl = false;
    let mut has_crlf = false;
    let mut prev = 0u8;
    for &b in bytes {
        if b == b'\n' {
            has_nl = true;
            if prev == b'\r' {
                has_crlf = true;
            }
        }
        prev = b;
    }
    let eol = if !has_nl {
        EolStyle::None
    } else if has_crlf {
        EolStyle::Crlf
    } else {
        EolStyle::Lf
    };
    ByteEnvelope { bom, eol }
}

/// Ошибка разбора XML-дескриптора (типизированная, §1.0 — без best-effort).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlReadError {
    /// Токенайзер вернул синтаксическую ошибку.
    Malformed(String),
    /// Не UTF-8 / битая кодировка значения.
    Encoding(String),
    /// Документ без единственного корневого элемента.
    NoRoot,
    /// В документе встретилось то, что дескриптор объекта 1С не содержит
    /// (PI кроме декларации, DOCTYPE, CDATA, …) — явная ошибка, не skip.
    Unsupported(String),
    /// Встретилась XML-сущность, которую эмиттер НЕ воспроизводит byte-exact
    /// (числовая `&#…;`, либо именованная кроме `amp`/`lt`/`gt`/`quot` — напр.
    /// `&apos;`). Чтобы не получить не-round-trip-абельное значение, читаем ЯВНУЮ
    /// ошибку (§1.0: без lossy best-effort), а не молча декодим (M1).
    NonReproducibleEntity(String),
}

impl std::fmt::Display for XmlReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlReadError::Malformed(s) => write!(f, "malformed XML: {s}"),
            XmlReadError::Encoding(s) => write!(f, "encoding error: {s}"),
            XmlReadError::NoRoot => write!(f, "document has no root element"),
            XmlReadError::Unsupported(s) => write!(f, "unsupported XML construct: {s}"),
            XmlReadError::NonReproducibleEntity(s) => {
                write!(f, "non-byte-reproducible XML entity: {s}")
            }
        }
    }
}

impl std::error::Error for XmlReadError {}

/// СТРОГИЙ unescape (M1): принимает РОВНО `&amp;`/`&lt;`/`&gt;`/`&quot;` (сущности,
/// которые эмитит 1С и которые наш эмиттер воспроизводит byte-exact). `&gt;`/`&quot;`
/// принимаются ЛЕНИВО (независимо от формата): форматы РАСХОДЯТСЯ в их эмиссии (EDT
/// `.mdo` экранирует `"`→`&quot;` в тексте; Designer `.xml` оставляет `"` литеральным —
/// сверено корпусом), а эмиттер воспроизводит конвенцию формата через
/// [`crate::emit::Envelope`] (`escape_gt`/`escape_quot`). Любая ИНАЯ сущность (числовая
/// `&#…;` или `&apos;`/прочее) → [`XmlReadError::NonReproducibleEntity`] — не получить
/// значение, которое writer не вернёт побайтно (§1.0).
fn strict_unescape(raw: &str) -> Result<String, XmlReadError> {
    // ТЕКСТ элемента: `&quot;` РЕПРОДУЦИРУЕМ (EDT-эмиттер его восстанавливает per-format).
    strict_unescape_inner(raw, /*allow_quot=*/ true)
}

/// `strict_unescape` для значения АТРИБУТА: `&quot;` ЗАПРЕЩЁН. Эмиттер пишет значения
/// атрибутов с литеральной кавычкой (escape_attr её не экранирует — uuid/ns/version/
/// xsi-type кавычек не несут), поэтому `&quot;` в атрибуте writer НЕ воспроизведёт
/// byte-exact → нерепродуцируемо (§1.0). В корпусе таких атрибутов нет.
fn strict_unescape_attr(raw: &str) -> Result<String, XmlReadError> {
    strict_unescape_inner(raw, /*allow_quot=*/ false)
}

/// Общий строгий unescape; `allow_quot` решает, репродуцируема ли `&quot;` в ДАННОМ
/// контексте (текст — да; атрибут — нет).
fn strict_unescape_inner(raw: &str, allow_quot: bool) -> Result<String, XmlReadError> {
    if !raw.contains('&') {
        return Ok(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            // Находим `;`.
            let semi = raw[i..].find(';').map(|off| i + off).ok_or_else(|| {
                XmlReadError::Malformed(format!("unterminated entity in {raw:?}"))
            })?;
            let entity = &raw[i + 1..semi]; // между `&` и `;`
            match entity {
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" if allow_quot => out.push('"'),
                other => return Err(XmlReadError::NonReproducibleEntity(format!("&{other};"))),
            }
            i = semi + 1;
        } else {
            // Берём один UTF-8 символ целиком.
            let ch = raw[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    Ok(out)
}

/// Разбить квалифицированное имя `prefix:local` → `(prefix, local)`.
fn split_qname(qname: &[u8]) -> Result<(String, String), XmlReadError> {
    let s = std::str::from_utf8(qname).map_err(|e| XmlReadError::Encoding(e.to_string()))?;
    match s.split_once(':') {
        Some((p, l)) => Ok((p.to_string(), l.to_string())),
        None => Ok((String::new(), s.to_string())),
    }
}

/// Построить [`Element`] (без детей) из start/empty-тега: имя + атрибуты.
fn element_from_start(e: &quick_xml::events::BytesStart<'_>) -> Result<Element, XmlReadError> {
    let (prefix, local) = split_qname(e.name().as_ref())?;
    let mut el = Element::new(prefix, local);
    for a in e.attributes() {
        let a = a.map_err(|err| XmlReadError::Malformed(err.to_string()))?;
        let name = std::str::from_utf8(a.key.as_ref())
            .map_err(|err| XmlReadError::Encoding(err.to_string()))?
            .to_string();
        // Сырые (ещё экранированные) байты значения → строгий unescape АТРИБУТА (M1):
        // только `&amp;`/`&lt;`/`&gt;` (БЕЗ `&quot;` — атрибуты эмитятся с литеральной
        // кавычкой), иначе ошибка (не permissive quick-xml).
        let raw = std::str::from_utf8(a.value.as_ref())
            .map_err(|err| XmlReadError::Encoding(err.to_string()))?;
        let value = strict_unescape_attr(raw)?;
        el.attrs.push(Attr {
            name,
            value,
            claimed: std::cell::Cell::new(false),
        });
    }
    Ok(el)
}

thread_local! {
    /// Читать ли IN-TEXT перевод строки ДОСЛОВНО (не схлопывая CRLF в LF).
    ///
    /// Дефолт `false` — прежнее поведение (канонизация к LF), под которым витнессен ВЕСЬ
    /// EDT-путь: EDT-дамп пишет перевод строки ВНУТРИ значения как CRLF (SSL: 707 узлов с
    /// in-text CRLF против 0 с голым LF), а Designer-дамп ТОГО ЖЕ конфига — как LF (971
    /// против 4, и те 4 — base64-блобы макетов) ⇒ у EDT это КОНВЕНЦИЯ ДИАЛЕКТА, и
    /// схлопывание обязано остаться.
    ///
    /// `true` ставит DESIGNER-путь чтения, потому что там это НЕ конвенция, а ДАННЫЕ: конфиг
    /// `integration_subsystem` несёт в Designer-дампе `<v8:content>` c CRLF ВНУТРИ значения, и
    /// платформенный `.cf` этого же конфига хранит там CR + CRLF, то есть СОХРАНЯЕТ данные-CR
    /// (у SSL, где данные — голый LF, в cf лежит ровно CRLF — сверено на `ssl.cf`). Канонизация
    /// съедала этот CR, и наш cf расходился с эталоном на 26 узлах CompareCfg (6 подсказок +
    /// 5 текстов запроса).
    static VERBATIM_IN_TEXT_EOL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Выполнить `f`, читая IN-TEXT переводы строк ДОСЛОВНО (см. `VERBATIM_IN_TEXT_EOL`).
/// Ставится ридером диалекта Designer на всё время чтения конфигурации.
pub fn with_verbatim_in_text_eol<R>(f: impl FnOnce() -> R) -> R {
    let prev = VERBATIM_IN_TEXT_EOL.with(|c| c.replace(true));
    let out = f();
    VERBATIM_IN_TEXT_EOL.with(|c| c.set(prev));
    out
}

/// Текущее значение политики in-text EOL — чтобы ЗАХВАТИТЬ его перед fan-out'ом в пул
/// (rayon НЕ наследует thread_local; ровно та же оговорка, что у ambient roundtrip-таргета).
pub fn in_text_eol_verbatim() -> bool {
    VERBATIM_IN_TEXT_EOL.with(|c| c.get())
}

/// Переустановить ЗАХВАЧЕННОЕ значение политики в рабочем потоке пула.
pub fn with_captured_in_text_eol<R>(verbatim: bool, f: impl FnOnce() -> R) -> R {
    let prev = VERBATIM_IN_TEXT_EOL.with(|c| c.replace(verbatim));
    let out = f();
    VERBATIM_IN_TEXT_EOL.with(|c| c.set(prev));
    out
}

/// Разобрать байты дескриптора в [`Descriptor`].
///
/// §1.0: любое неожиданное событие (DOCTYPE/CDATA/PI кроме декларации) → ошибка.
/// Текст узла unescape'ится; whitespace-only текст между элементами отбрасывается
/// (это форматирование envelope, восстанавливается эмиттером, не данные).
pub fn parse(bytes: &[u8]) -> Result<Descriptor, XmlReadError> {
    // M2: наблюдаем байтовую обёртку (BOM/EOL) ДО токенизации — коннектор её сверит.
    let bytes_env = observe_envelope(bytes);

    let mut reader = Reader::from_reader(bytes);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;

    let mut decl: Option<String> = None;
    // Стек открытых элементов; вершина — текущий родитель.
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let mut buf = Vec::new();

    loop {
        let ev = reader.read_event_into(&mut buf).map_err(|e| match e {
            quick_xml::Error::NonDecodable(_) => XmlReadError::Encoding(e.to_string()),
            other => XmlReadError::Malformed(other.to_string()),
        })?;
        match ev {
            Event::Decl(d) => {
                // Сохраняем точные байты декларации (между `<?` и `?>`), чтобы при
                // записи восстановить пролог байт-в-байт.
                let inner = d.as_ref();
                let s = std::str::from_utf8(inner)
                    .map_err(|e| XmlReadError::Encoding(e.to_string()))?;
                decl = Some(format!("<?{}?>", s.trim_end()));
            }
            Event::Start(e) => {
                stack.push(element_from_start(&e)?);
            }
            Event::Empty(e) => {
                // Самозакрывающийся элемент: дети/текст пусты.
                let el = element_from_start(&e)?;
                push_completed(&mut stack, &mut root, el)?;
            }
            Event::End(_) => {
                let el = stack.pop().ok_or_else(|| {
                    XmlReadError::Malformed("end tag without matching start".into())
                })?;
                push_completed(&mut stack, &mut root, el)?;
            }
            Event::Text(t) => {
                // Сырые (ещё экранированные) байты текста → строгий unescape (M1).
                let raw_bytes = t.into_inner();
                let raw = std::str::from_utf8(raw_bytes.as_ref())
                    .map_err(|e| XmlReadError::Encoding(e.to_string()))?;
                let unescaped = strict_unescape(raw)?;
                // Whitespace между элементами — это форматирование (indent/EOL),
                // его восстанавливает эмиттер. Значимый текст листа сохраняем.
                //
                // ТОНКОСТЬ (whitespace-CONTENT листа, напр. `<value>` из 32 пробелов в
                // fill-значении). ПРЕЖНЕЕ правило («индентация == whitespace, содержащий
                // перевод строки») ОПРОВЕРГНУТО ERP: `Reports/АнализНачисленийИУдержаний.xml`
                // несёт `<v8:content>` РОВНО из одного перевода строки — значимый контент
                // (в эталонном cf он лежит как `{2,"ru","<CRLF>","en","<CRLF>"}`), а правило
                // молча его съедало. Индентацию от контента отличает НЕ перевод строки, а
                // НАЛИЧИЕ ДЕТЕЙ: текст копится всегда, а whitespace-only текст КОНТЕЙНЕРА
                // (у которого есть дочерние элементы) отбрасывается при закрытии элемента —
                // см. [`drop_indentation_text`]. Смешанного контента (текст + дети) в 1С-XML нет.
                if !unescaped.is_empty() {
                    // §1.6: КАНОНИЗАЦИЯ in-text EOL к `\n`. Многострочный текст
                    // (напр. `<v8:content>`/`<value>` пояснения) хранит перевод строки
                    // ВНУТРИ значения; EDT пишет его `\r\n`, Designer — `\n` (сверено
                    // корпусом). Нормализуем к `\n` → оба формата дают РАВНЫЙ IR-текст
                    // (X by construction); эмиттер восстанавливает per-format конвенцию.
                    let normalized = if VERBATIM_IN_TEXT_EOL.with(|c| c.get()) {
                        unescaped
                    } else {
                        unescaped.replace("\r\n", "\n")
                    };
                    if let Some(top) = stack.last_mut() {
                        top.text.push_str(&normalized);
                    } else if !normalized.trim().is_empty() {
                        // Пролог/эпилог документа: whitespace ВНЕ корня — это перевод строки
                        // после `<?xml …?>`, а не текст (значимый текст там = ошибка).
                        return Err(XmlReadError::Malformed(
                            "text outside of root element".into(),
                        ));
                    }
                }
            }
            Event::Eof => break,
            Event::Comment(_) => {
                return Err(XmlReadError::Unsupported("comment node".into()));
            }
            Event::CData(_) => {
                return Err(XmlReadError::Unsupported("CDATA section".into()));
            }
            Event::PI(_) => {
                return Err(XmlReadError::Unsupported("processing instruction".into()));
            }
            Event::DocType(_) => {
                return Err(XmlReadError::Unsupported("DOCTYPE".into()));
            }
        }
        buf.clear();
    }

    if !stack.is_empty() {
        return Err(XmlReadError::Malformed("unclosed element(s)".into()));
    }
    let root = root.ok_or(XmlReadError::NoRoot)?;
    Ok(Descriptor {
        bytes_env,
        decl,
        root,
    })
}

/// Прикрепить завершённый элемент к родителю на вершине стека либо сделать корнем.
/// Принимает `&mut [Element]` — родителя достаём через `last_mut`, сам стек не растим.
/// Отбросить межэлементную ИНДЕНТАЦИЮ: whitespace-only текст элемента, У КОТОРОГО ЕСТЬ
/// ДОЧЕРНИЕ ЭЛЕМЕНТЫ, — это форматирование (его восстанавливает эмиттер). У ЛИСТА тот же
/// текст — ЗНАЧИМОЕ содержимое (`<value>` из пробелов; `<v8:content>` из одного перевода
/// строки — witnessed ERP) и сохраняется.
fn drop_indentation_text(el: &mut Element) {
    if !el.children.is_empty() && el.text.trim().is_empty() {
        el.text.clear();
    }
}

fn push_completed(
    stack: &mut [Element],
    root: &mut Option<Element>,
    mut el: Element,
) -> Result<(), XmlReadError> {
    drop_indentation_text(&mut el);
    match stack.last_mut() {
        Some(parent) => {
            parent.children.push(el);
            Ok(())
        }
        None => {
            if root.is_some() {
                return Err(XmlReadError::Malformed("multiple root elements".into()));
            }
            *root = Some(el);
            Ok(())
        }
    }
}
