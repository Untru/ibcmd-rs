//! XDTO-schema codec: содержимое текстового XML-спутника `Package.xdto` (EDT) /
//! `Ext/Package.bin` (Designer) ↔ канонический рекурсивный [`XdtoNode`]-tree, общий
//! субстрат среза S4 (XDTOPackage).
//!
//! # Зачем ЗДЕСЬ (formats-xml)
//! Схема XDTO-пакета хранится ОТДЕЛЬНЫМ XML-файлом рядом с тонким дескриптором `.mdo`/`.xml`
//! (root `<package …>`, ns `http://v8.1c.ru/8.1/xdto`): РЕКУРСИВНОЕ дерево `objectType`/
//! `valueType`/`property`/`typeDef`/`enumeration`/`pattern`/`import` с богатым набором
//! атрибутов (`type`/`form`/`upperBound`/`lowerBound`/`nillable`/`base`/`variety`/
//! `memberTypes`/`maxLength`/… + локальные namespace-alias'ы `xmlns:d2p1`…). И EDT, и
//! Designer держат ОДНО И ТО ЖЕ тело — сверено корпусом SSL: тело байт-идентично, различаясь
//! ЛИШЬ envelope-обёрткой (EDT — без BOM; Designer — с BOM; `<?xml?>`-декларации нет ни у
//! кого). Поэтому парсинг тела — ОБЩИЙ, а BOM — параметр ([`SidecarFormat`]): обе стороны
//! дают РАВНЫЙ [`XdtoNode`]-tree → cross-format X by construction (§1.6/§3.5), а R остаётся
//! byte-exact для КАЖДОГО формата (каждый регенерирует СВОЙ envelope).
//!
//! # §1.0-тотальность (не best-effort, не opaque-Blob)
//! Схема XDTO — фактически полный XSD-подобный граммар (8 видов элементов, 30+ атрибутов,
//! текстовые узлы `<pattern>`/`<enumeration>` с regex и XML-entity, per-элементные ns-alias).
//! Захватывать его закрытой таблицей полей было бы неполно (§1.0 требует потребить КАЖДЫЙ
//! элемент/атрибут). Поэтому IR — ОБОБЩЁННОЕ рекурсивное дерево XML-узлов, потребляющее ВСЮ
//! структуру: имя элемента, УПОРЯДОЧЕННЫЕ атрибуты (имя+значение, включая ns-alias-объявления),
//! УПОРЯДОЧЕННЫЕ дети (элементы + текстовые прогоны). Это НЕ opaque-Blob: парсер — настоящий
//! рекурсивный спуск, проверяющий корректную вложенность/закрытие тегов; ЛЮБАЯ малформация
//! (незакрытый тег, `<` в имени, обрыв) → типизированная [`XdtoError`], а не проглатывание.
//!
//! # Byte-exact re-emission
//! Текстовые прогоны между тегами (CRLF + TAB-отступы, а также содержимое `<pattern>`/
//! `<enumeration>`) сохраняются ВЕРБАТИМ отдельными текстовыми детьми, а атрибуты — с их
//! ТОЧНЫМ исходным написанием (raw-байты значения между кавычками, кавычка запомнена).
//! Эмиссия конкатенирует всё дословно → `write(read(x)) == x` byte-exact by construction, без
//! домысла отступов из глубины дерева. Форма фиксирована: `<`, имя, атрибуты (` name="val"`),
//! либо `/>` (пустой), либо `>` + дети + `</имя>`.

use crate::registry::SidecarFormat;

/// Типизированная ошибка XDTO-schema-кодека (§1.0 — без best-effort/skip).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdtoError(pub String);

impl std::fmt::Display for XdtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "xdto sidecar: {}", self.0)
    }
}

impl std::error::Error for XdtoError {}

fn err<T>(msg: impl Into<String>) -> Result<T, XdtoError> {
    Err(XdtoError(msg.into()))
}

const BOM: &[u8] = b"\xef\xbb\xbf";

impl SidecarFormat {
    /// Есть ли у формата BOM в начале файла XDTO-спутника (Designer — да; EDT — нет).
    fn xdto_has_bom(self) -> Result<bool, XdtoError> {
        match self {
            SidecarFormat::EdtXdto => Ok(false),
            SidecarFormat::DesignerXdto => Ok(true),
            other => err(format!("not an XDTO sidecar format: {other:?}")),
        }
    }
}

// --- Канонический IR: рекурсивное дерево XML-узлов ------------------------------------

/// Один атрибут элемента (`name="value"`), сохранённый ВЕРБАТИМ для byte-exact re-emission.
///
/// Включает ns-alias-объявления (`xmlns:d2p1="…"`) — они синтаксически такие же атрибуты и
/// потребляются наравне с прочими (§1.0). `value` — raw-байты МЕЖДУ кавычками (XML-entity
/// НЕ раскрываются: в корпусе значения содержат `{…}`/`\`/regex — их дословное сохранение
/// и есть byte-exact). `quote` запоминает символ кавычки (в корпусе — всегда `"`, но
/// сохраняем для строгой реконструкции).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdtoAttr {
    /// Имя атрибута с префиксом как в источнике (`type`, `xsi:type`, `xmlns:d2p1`, …).
    pub name: String,
    /// Значение — raw-байты между кавычками (дословно, без раскрытия entity).
    pub value: String,
    /// Разделяющее пробельное содержимое ПЕРЕД этим атрибутом (обычно один пробел ` `),
    /// сохранённое дословно для byte-exact (в корпусе — ровно ` `, но не хардкодим).
    pub lead_ws: String,
    /// Символ кавычки значения (`"` или `'`).
    pub quote: u8,
}

/// Узел XDTO-дерева: либо элемент (с детьми), либо текстовый прогон (verbatim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XdtoNode {
    /// Элемент `<name attrs>…children…</name>` либо `<name attrs/>` (пустой).
    Element {
        /// Имя тега с префиксом как в источнике (`package`, `objectType`, `property`, …).
        name: String,
        /// Атрибуты В ПОРЯДКЕ источника (byte-exact).
        attrs: Vec<XdtoAttr>,
        /// Пробельное содержимое между последним атрибутом (или именем) и `>`/`/>`
        /// (в корпусе — пусто, но сохраняем дословно).
        pre_close_ws: String,
        /// `true` → самозакрывающийся `<…/>` (без детей и без `</name>`).
        self_closing: bool,
        /// Дети В ПОРЯДКЕ источника (элементы вперемешку с текстовыми прогонами).
        children: Vec<XdtoNode>,
    },
    /// Текстовый прогон между тегами (CRLF/TAB-отступы, содержимое `<pattern>` и т.п.),
    /// сохранённый ВЕРБАТИМ (raw-байты, entity не раскрываются).
    Text(String),
}

/// Разобранный XDTO-спутник: BOM-флаг envelope (для byte-exact R) + корневой узел `<package>`.
///
/// Тело (root + всё под ним) формат-нейтрально: EDT и Designer парсят РАВНЫЙ `root` (X). BOM
/// — единственное envelope-различие, поэтому НЕ часть `root` (иначе X ложно разошёлся бы) —
/// он несётся отдельно и НЕ участвует в cross-format сравнении (см. [`bodies_equal`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdtoPackage {
    /// Был ли BOM в источнике (EDT — false, Designer — true). Часть R-envelope, НЕ X-тела.
    pub had_bom: bool,
    /// Корневой узел `<package …>` (со всей рекурсивной схемой).
    pub root: XdtoNode,
}

// --- READ: байты спутника → XdtoPackage -----------------------------------------------

/// Курсор строгого спуска по байтам тела. §1.0: любое отклонение формы → типизированная
/// ошибка с точным смещением.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn peek_byte(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Потребить точный литерал или ошибка.
    fn expect(&mut self, lit: &[u8], ctx: &str) -> Result<(), XdtoError> {
        if self.rest().starts_with(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            let got = &self.rest()[..lit.len().min(self.rest().len()).min(32)];
            err(format!(
                "at offset {}: expected {ctx} {:?}, found {:?}",
                self.pos,
                String::from_utf8_lossy(lit),
                String::from_utf8_lossy(got)
            ))
        }
    }
}

/// Байт — часть имени XML-тега/атрибута (letters/digits/`.`/`-`/`_`/`:`). Мы не различаем
/// namespaces структурно — имя хранится с префиксом дословно.
fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':')
        // XDTO-имена могут быть НЕ-ASCII (кириллица в EnterpriseData): любой не-разделитель.
        || b >= 0x80
}

/// Байт — XML-пробел.
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// Прочитать имя (тега/атрибута) с текущей позиции. Ошибка, если пусто.
fn read_name(c: &mut Cursor, ctx: &str) -> Result<String, XdtoError> {
    let start = c.pos;
    while c.pos < c.data.len() && is_name_byte(c.data[c.pos]) {
        c.pos += 1;
    }
    if c.pos == start {
        return err(format!("at offset {}: empty {ctx} name", c.pos));
    }
    utf8(&c.data[start..c.pos], ctx, start)
}

/// Прочитать пробельный прогон (может быть пустым), вернуть его дословно.
fn read_ws(c: &mut Cursor) -> Result<String, XdtoError> {
    let start = c.pos;
    while c.pos < c.data.len() && is_ws(c.data[c.pos]) {
        c.pos += 1;
    }
    utf8(&c.data[start..c.pos], "whitespace", start)
}

fn utf8(bytes: &[u8], ctx: &str, at: usize) -> Result<String, XdtoError> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|e| XdtoError(format!("at offset {at}: non-utf8 {ctx}: {e}")))
}

/// Прочитать один атрибут `name="value"` (lead_ws уже прочитан вызывающим и передан).
fn read_attr(c: &mut Cursor, lead_ws: String) -> Result<XdtoAttr, XdtoError> {
    let name = read_name(c, "attribute")?;
    c.expect(b"=", "attribute '='")?;
    let quote = c
        .peek_byte()
        .filter(|&b| b == b'"' || b == b'\'')
        .ok_or_else(|| XdtoError(format!("at offset {}: expected attribute quote", c.pos)))?;
    c.pos += 1; // opening quote
    let vstart = c.pos;
    while c.pos < c.data.len() && c.data[c.pos] != quote {
        c.pos += 1;
    }
    if c.pos >= c.data.len() {
        return err(format!("at offset {vstart}: unterminated attribute value"));
    }
    let value = utf8(&c.data[vstart..c.pos], "attribute value", vstart)?;
    c.pos += 1; // closing quote
    Ok(XdtoAttr {
        name,
        value,
        lead_ws,
        quote,
    })
}

/// Прочитать открывающий/самозакрывающийся тег: `<` уже потреблён вызывающим. Возвращает
/// `(name, attrs, pre_close_ws, self_closing)`.
fn read_open_tag(c: &mut Cursor) -> Result<(String, Vec<XdtoAttr>, String, bool), XdtoError> {
    let name = read_name(c, "element")?;
    let mut attrs = Vec::new();
    loop {
        let ws = read_ws(c)?;
        match c.peek_byte() {
            Some(b'/') => {
                c.expect(b"/>", "self-closing '/>'")?;
                return Ok((name, attrs, ws, true));
            }
            Some(b'>') => {
                c.pos += 1;
                return Ok((name, attrs, ws, false));
            }
            Some(_) => {
                // Атрибут: `ws` — его lead-пробел (обязан быть непуст, чтобы отделить).
                if ws.is_empty() {
                    return err(format!(
                        "at offset {}: expected whitespace before attribute in <{name}>",
                        c.pos
                    ));
                }
                attrs.push(read_attr(c, ws)?);
            }
            None => return err(format!("unterminated <{name}> tag")),
        }
    }
}

/// Прочитать содержимое элемента (детей) до его закрывающего тега `</name>`. `<` первого
/// потомка ещё не потреблён. Возвращает список детей (текстовые прогоны + вложенные элементы).
fn read_children(c: &mut Cursor, name: &str) -> Result<Vec<XdtoNode>, XdtoError> {
    let mut children = Vec::new();
    loop {
        // Текстовый прогон до следующего `<` (может быть пустым — тогда не добавляем узел).
        let tstart = c.pos;
        while c.pos < c.data.len() && c.data[c.pos] != b'<' {
            c.pos += 1;
        }
        if c.pos > tstart {
            children.push(XdtoNode::Text(utf8(
                &c.data[tstart..c.pos],
                "text",
                tstart,
            )?));
        }
        if c.at_end() {
            return err(format!(
                "unexpected end of input inside <{name}> (no </{name}>)"
            ));
        }
        // Здесь `data[pos] == '<'`. Закрывающий тег?
        if c.rest().starts_with(b"</") {
            c.pos += 2;
            let close = read_name(c, "closing tag")?;
            if close != name {
                return err(format!(
                    "at offset {}: mismatched closing </{close}>, expected </{name}>",
                    c.pos
                ));
            }
            // Допускаем пробел перед `>` в закрывающем теге (в корпусе нет, но строго читаем).
            let _ = read_ws(c)?;
            c.expect(b">", "closing '>'")?;
            return Ok(children);
        }
        // Reject comments/PI/CDATA explicitly (§1.0 — не глотаем неизвестное).
        if c.rest().starts_with(b"<!") || c.rest().starts_with(b"<?") {
            return err(format!(
                "at offset {}: unsupported XML construct (comment/PI/CDATA) in XDTO schema",
                c.pos
            ));
        }
        // Вложенный элемент.
        c.pos += 1; // '<'
        children.push(read_element(c)?);
    }
}

/// Прочитать один элемент целиком (открывающий `<` уже потреблён вызывающим).
fn read_element(c: &mut Cursor) -> Result<XdtoNode, XdtoError> {
    let (name, attrs, pre_close_ws, self_closing) = read_open_tag(c)?;
    if self_closing {
        return Ok(XdtoNode::Element {
            name,
            attrs,
            pre_close_ws,
            self_closing: true,
            children: Vec::new(),
        });
    }
    let children = read_children(c, &name)?;
    Ok(XdtoNode::Element {
        name,
        attrs,
        pre_close_ws,
        self_closing: false,
        children,
    })
}

/// Прочитать байты XDTO-спутника в [`XdtoPackage`], СВЕРЯЯ BOM-envelope формата `fmt`.
/// §1.0: BOM обязан соответствовать формату; корень обязан быть `<package>`; хвостовые
/// не-пробельные байты после `</package>` → ошибка.
pub fn read(bytes: &[u8], fmt: SidecarFormat) -> Result<XdtoPackage, XdtoError> {
    let want_bom = fmt.xdto_has_bom()?;
    let mut c = Cursor::new(bytes);
    let had_bom = c.rest().starts_with(BOM);
    if had_bom != want_bom {
        return err(format!(
            "BOM mismatch: format {fmt:?} expects BOM={want_bom}, source has BOM={had_bom}"
        ));
    }
    if had_bom {
        c.pos += BOM.len();
    }
    // Корень: `<package …>` (единственный top-level элемент; нет `<?xml?>`).
    c.expect(b"<", "root '<'")?;
    let root = read_element(&mut c)?;
    match &root {
        XdtoNode::Element { name, .. } if name == "package" => {}
        XdtoNode::Element { name, .. } => {
            return err(format!("root element is <{name}>, expected <package>"))
        }
        XdtoNode::Text(_) => return err("root is text, expected <package>"),
    }
    // Хвост: допускаются лишь пробелы (в корпусе — ничего). Не-пробел → §1.0-ошибка.
    let tail = read_ws(&mut c)?;
    if !c.at_end() {
        return err(format!(
            "at offset {}: trailing bytes after </package>: {tail:?}...",
            c.pos
        ));
    }
    // Если хвост непуст (одни пробелы) — он потерялся бы при эмиссии. В корпусе пусто;
    // строго запрещаем, чтобы не молча «поправить» (§1.0).
    if !tail.is_empty() {
        return err(
            "trailing whitespace after </package> not representable byte-exact".to_string(),
        );
    }
    Ok(XdtoPackage { had_bom, root })
}

// --- WRITE: XdtoPackage → байты спутника (byte-exact) ---------------------------------

/// Регенерировать байты спутника из [`XdtoPackage`] под envelope формата `fmt` (byte-exact с
/// источником, если IR получен `read(…, fmt)`). BOM берётся из `fmt` (сверяется с `had_bom`).
pub fn write(pkg: &XdtoPackage, fmt: SidecarFormat) -> Result<Vec<u8>, XdtoError> {
    let want_bom = fmt.xdto_has_bom()?;
    if pkg.had_bom != want_bom {
        return err(format!(
            "BOM/format mismatch on write: format {fmt:?} expects BOM={want_bom}, IR has BOM={}",
            pkg.had_bom
        ));
    }
    let mut out = Vec::new();
    if want_bom {
        out.extend_from_slice(BOM);
    }
    emit_node(&pkg.root, &mut out);
    Ok(out)
}

/// Дословно сериализовать узел в байты (byte-exact реконструкция формы источника).
fn emit_node(node: &XdtoNode, out: &mut Vec<u8>) {
    match node {
        XdtoNode::Text(t) => out.extend_from_slice(t.as_bytes()),
        XdtoNode::Element {
            name,
            attrs,
            pre_close_ws,
            self_closing,
            children,
        } => {
            out.push(b'<');
            out.extend_from_slice(name.as_bytes());
            for a in attrs {
                out.extend_from_slice(a.lead_ws.as_bytes());
                out.extend_from_slice(a.name.as_bytes());
                out.push(b'=');
                out.push(a.quote);
                out.extend_from_slice(a.value.as_bytes());
                out.push(a.quote);
            }
            out.extend_from_slice(pre_close_ws.as_bytes());
            if *self_closing {
                out.extend_from_slice(b"/>");
            } else {
                out.push(b'>');
                for ch in children {
                    emit_node(ch, out);
                }
                out.extend_from_slice(b"</");
                out.extend_from_slice(name.as_bytes());
                out.push(b'>');
            }
        }
    }
}

// --- X: cross-format сравнение тела ----------------------------------------------------

/// Cross-format равенство ТЕЛА двух XDTO-пакетов (§3.5): игнорирует envelope-BOM (формат-
/// локальный), сравнивает рекурсивный корневой узел. Тело байт-идентично у форматов (сверено
/// корпусом), поэтому деревья ОБЯЗАНЫ совпасть.
pub fn bodies_equal(a: &XdtoPackage, b: &XdtoPackage) -> bool {
    a.root == b.root
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Реальный минимальный образец тела (CurrencyRates-подобный): root + один objectType.
    const BODY: &[u8] = b"<package xmlns=\"http://v8.1c.ru/8.1/xdto\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" targetNamespace=\"http://www.1c.ru/SaaS/SuppliedData/CurrencyRates\">\r\n\t<objectType name=\"Rate\">\r\n\t\t<property name=\"Code\" type=\"xs:string\" lowerBound=\"1\" form=\"Attribute\"/>\r\n\t</objectType></package>";

    fn edt_bytes() -> Vec<u8> {
        BODY.to_vec()
    }

    fn designer_bytes() -> Vec<u8> {
        let mut v = Vec::from(BOM);
        v.extend_from_slice(BODY);
        v
    }

    #[test]
    fn edt_roundtrips_byte_exact() {
        let src = edt_bytes();
        let pkg = read(&src, SidecarFormat::EdtXdto).unwrap();
        assert!(!pkg.had_bom);
        assert_eq!(
            write(&pkg, SidecarFormat::EdtXdto).unwrap(),
            src,
            "EDT byte-exact"
        );
    }

    #[test]
    fn designer_roundtrips_byte_exact() {
        let src = designer_bytes();
        let pkg = read(&src, SidecarFormat::DesignerXdto).unwrap();
        assert!(pkg.had_bom);
        assert_eq!(
            write(&pkg, SidecarFormat::DesignerXdto).unwrap(),
            src,
            "Designer byte-exact"
        );
    }

    #[test]
    fn cross_format_bodies_equal() {
        let e = read(&edt_bytes(), SidecarFormat::EdtXdto).unwrap();
        let d = read(&designer_bytes(), SidecarFormat::DesignerXdto).unwrap();
        assert!(
            bodies_equal(&e, &d),
            "EDT/Designer bodies must be identical (X)"
        );
        assert_ne!(
            e.had_bom, d.had_bom,
            "envelope BOM differs (not part of body)"
        );
    }

    #[test]
    fn parses_recursive_tree_and_attrs() {
        let pkg = read(&edt_bytes(), SidecarFormat::EdtXdto).unwrap();
        let XdtoNode::Element {
            name,
            attrs,
            children,
            self_closing,
            ..
        } = &pkg.root
        else {
            panic!("root not element");
        };
        assert_eq!(name, "package");
        assert!(!self_closing);
        assert_eq!(attrs[0].name, "xmlns");
        assert_eq!(attrs[3].name, "targetNamespace");
        // children: Text("\r\n\t"), Element(objectType)
        let obj = children.iter().find_map(|n| match n {
            XdtoNode::Element { name, children, .. } if name == "objectType" => Some(children),
            _ => None,
        });
        let obj = obj.expect("objectType present");
        let prop = obj.iter().find_map(|n| match n {
            XdtoNode::Element {
                name,
                attrs,
                self_closing,
                ..
            } if name == "property" => Some((attrs, self_closing)),
            _ => None,
        });
        let (pattrs, psc) = prop.expect("property present");
        assert!(*psc, "property is self-closing");
        assert_eq!(
            pattrs.iter().find(|a| a.name == "type").unwrap().value,
            "xs:string"
        );
        assert_eq!(
            pattrs.iter().find(|a| a.name == "form").unwrap().value,
            "Attribute"
        );
    }

    #[test]
    fn rejects_bom_mismatch() {
        assert!(
            read(&designer_bytes(), SidecarFormat::EdtXdto).is_err(),
            "EDT must reject BOM"
        );
        assert!(
            read(&edt_bytes(), SidecarFormat::DesignerXdto).is_err(),
            "Designer must require BOM"
        );
    }

    #[test]
    fn rejects_wrong_root() {
        let src = b"<notpackage/>".to_vec();
        assert!(
            read(&src, SidecarFormat::EdtXdto).is_err(),
            "non-<package> root → error"
        );
    }

    #[test]
    fn rejects_mismatched_close() {
        let src = b"<package><objectType></package>".to_vec();
        assert!(
            read(&src, SidecarFormat::EdtXdto).is_err(),
            "mismatched close → error"
        );
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut src = edt_bytes();
        src.extend_from_slice(b"garbage");
        assert!(
            read(&src, SidecarFormat::EdtXdto).is_err(),
            "trailing bytes → error"
        );
    }

    #[test]
    fn preserves_text_content_and_entities() {
        // pattern/enumeration text with regex backslashes, braces, and &amp; entity verbatim.
        let src = b"<package><valueType name=\"x\"><pattern>[\\da-f]{8}&amp;z</pattern></valueType></package>".to_vec();
        let pkg = read(&src, SidecarFormat::EdtXdto).unwrap();
        assert_eq!(
            write(&pkg, SidecarFormat::EdtXdto).unwrap(),
            src,
            "text/entities verbatim"
        );
    }
}
