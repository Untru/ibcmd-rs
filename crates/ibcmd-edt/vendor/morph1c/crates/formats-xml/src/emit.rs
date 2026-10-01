//! Byte-exact структурный эмиттер XML-дескриптора (§3.2 — формат детерминирован).
//!
//! НЕ универсальный XML-writer: те не дают byte-exact (indent/EOL/BOM/порядок
//! атрибутов/самозакрытие/экранирование расходятся с эталоном 1С). Здесь — маленький
//! детерминированный рендерер, параметризованный [`Envelope`]-константами формата
//! (BOM/EOL/indent/пролог). Дерево строится проекцией (через [`OutElement`]),
//! затем рендерится в точные байты.
//!
//! ВАЖНО (§1.1): эмиттер ВОССТАНАВЛИВАЕТ дескриптор из IR+спека+envelope, а НЕ
//! эхает вход. Поэтому R (`write(read(f))==f`) реально доказывает полноту IR.

/// Envelope-константы XML-формата: байтовая обёртка, не зависящая от конкретного
/// объекта. Проверяется на чтении, воспроизводится на записи (§3.2).
#[derive(Debug, Clone)]
pub struct Envelope {
    /// Писать ли UTF-8 BOM в начале (EDT: нет; Designer: да).
    pub bom: bool,
    /// Перевод строки (EDT: `"\r\n"` — подтверждено hexdump'ом; Designer: `"\r\n"`).
    pub eol: &'static str,
    /// Единица отступа (EDT: `"  "` 2 пробела; Designer: `"\t"`).
    pub indent_unit: &'static str,
    /// Точные байты XML-декларации без EOL (`<?xml version="1.0" encoding="UTF-8"?>`).
    pub decl: &'static str,
    /// Завершать ли документ переводом строки после корня (EDT: да — подтверждено).
    pub trailing_eol: bool,
    /// Экранировать ли `>` → `&gt;` в ТЕКСТЕ/атрибутах. Форматы РАСХОДЯТСЯ (сверено
    /// корпусом): EDT `.mdo` оставляет `>` ЛИТЕРАЛЬНЫМ (`Отлично (>=0.95)`), Designer
    /// `.xml` экранирует (`&gt;`). `&` и `<` экранируют ОБА. Read-side `strict_unescape`
    /// принимает `&gt;` всегда (лениво) → оба формата дают РАВНЫЙ IR (§1.6).
    pub escape_gt: bool,
    /// Экранировать ли `"` → `&quot;` в ТЕКСТЕ элемента. Форматы РАСХОДЯТСЯ (сверено
    /// корпусом): EDT `.mdo` экранирует кавычку В ТЕКСТЕ (`признак &quot;Рассмотрено&quot;`),
    /// Designer `.xml` оставляет `"` ЛИТЕРАЛЬНОЙ. Read-side `strict_unescape` принимает
    /// `&quot;` всегда (лениво) → оба формата дают РАВНЫЙ IR-текст (§1.6); эмиттер
    /// восстанавливает per-format конвенцию. На значения АТРИБУТОВ не влияет (uuid/ns/
    /// version кавычек не несут).
    pub escape_quot: bool,
    /// IN-TEXT перевод строки формата: канонический IR-текст хранит `\n`; при эмиссии
    /// многострочного значения `\n` разворачивается В ЭТО (EDT `"\r\n"`, Designer
    /// `"\n"` — сверено корпусом). Это НЕ структурный [`Envelope::eol`] (тот — между
    /// узлами), а конвенция перевода строки ВНУТРИ текстового значения (§1.6).
    pub text_eol: &'static str,
}

/// Узел выходного дерева, который строит проекция при записи.
///
/// Сознательно отдельный от read-side [`crate::descriptor::Element`]: write-дерево
/// не нуждается в учёте «claimed» и хранит уже сырой (не-escaped) текст, который
/// эмиттер экранирует при рендере.
#[derive(Debug, Clone)]
pub struct OutElement {
    /// Префикс ns (`""` = без префикса).
    pub prefix: String,
    /// Local-name тега.
    pub local: String,
    /// Атрибуты в порядке эмиссии `(имя, значение)` (значение — сырое, экранируется).
    pub attrs: Vec<(String, String)>,
    /// Дочерние элементы.
    pub children: Vec<OutElement>,
    /// Текст листа (сырой; экранируется при рендере). Непустой ⇒ лист.
    pub text: Option<String>,
    /// Эмитить как самозакрывающийся (`<tag/>`), даже без текста/детей.
    pub self_closing: bool,
    /// Private typed QName dialect facet; never rendered as XML.
    pub(crate) source_type_qname_native: Option<bool>,
}

impl OutElement {
    /// Контейнер/ветка: имя + ns, без текста.
    pub fn branch(prefix: impl Into<String>, local: impl Into<String>) -> Self {
        OutElement {
            prefix: prefix.into(),
            local: local.into(),
            attrs: Vec::new(),
            children: Vec::new(),
            text: None,
            self_closing: false,
            source_type_qname_native: None,
        }
    }

    /// Лист с текстом.
    pub fn leaf(
        prefix: impl Into<String>,
        local: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        let mut e = Self::branch(prefix, local);
        e.text = Some(text.into());
        e
    }

    /// Самозакрывающийся пустой элемент (`<tag/>`) — без текста/детей. Конвенция 1С
    /// для пустого текстового элемента (напр. пустой `<Comment/>` Designer-дескриптора).
    pub fn self_closing(prefix: impl Into<String>, local: impl Into<String>) -> Self {
        let mut e = Self::branch(prefix, local);
        e.self_closing = true;
        e
    }

    /// Добавить атрибут (порядок сохраняется).
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push((name.into(), value.into()));
        self
    }

    /// Добавить ребёнка.
    pub fn push(&mut self, child: OutElement) {
        self.children.push(child);
    }

    /// Полное имя тега (`prefix:local` или `local`).
    fn qname(&self) -> String {
        if self.prefix.is_empty() {
            self.local.clone()
        } else {
            format!("{}:{}", self.prefix, self.local)
        }
    }
}

/// Экранирование ТЕКСТА элемента под фактическую конвенцию 1С (M1).
///
/// Сводка ВСЕГО Designer-корпуса SSL: 1С использует РОВНО три сущности — `&`→`&amp;`,
/// `<`→`&lt;`, `>`→`&gt;` — и НИКОГДА числовые `&#…;`, `&quot;`, `&apos;`. Кавычки в
/// тексте 1С оставляет литеральными (валидны в текстовом узле). Симметрично:
/// [`crate::read::strict_unescape`] принимает ровно эти три и ошибается на иных →
/// read↔write byte-exact гарантирован (§1.0: иначе ОШИБКА, не lossy).
pub fn escape_text(s: &str) -> String {
    escape_with(s, /*escape_gt=*/ true)
}

/// Экранирование текста/атрибута с явной политикой по `>` (`escape_gt`). `&`/`<`
/// экранируются всегда; `>` — только при `escape_gt` (Designer=true, EDT=false; сверено
/// корпусом). Кавычку НЕ трогает (для значений атрибутов — uuid/ns/version их не несут).
pub fn escape_with(s: &str, escape_gt: bool) -> String {
    escape_text_with(s, escape_gt, /*escape_quot=*/ false)
}

/// Экранирование ТЕКСТА элемента с политиками по `>` и `"`. `&`/`<` всегда; `>` при
/// `escape_gt`; `"` при `escape_quot` (EDT=true → `&quot;` в тексте; Designer=false →
/// литеральная кавычка). Симметрично read-side `strict_unescape` (принимает оба лениво).
pub fn escape_text_with(s: &str, escape_gt: bool, escape_quot: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' if escape_gt => out.push_str("&gt;"),
            '"' if escape_quot => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Экранирование значения АТРИБУТА (атрибуты дескриптора 1С — uuid/xmlns/version —
/// кавычек/`>` не содержат; политика `>` берётся из envelope). Симметрично со
/// `strict_unescape`.
pub fn escape_attr(s: &str) -> String {
    escape_with(s, /*escape_gt=*/ true)
}

/// Срендерить дескриптор (пролог + корень) в точные байты согласно [`Envelope`].
pub fn render(env: &Envelope, root: &OutElement) -> Vec<u8> {
    let mut out = String::new();
    if env.bom {
        out.push('\u{feff}');
    }
    out.push_str(env.decl);
    out.push_str(env.eol);
    render_element(env, root, 0, &mut out);
    if env.trailing_eol {
        // render_element НЕ ставит EOL после закрытия корня — ставим здесь.
        out.push_str(env.eol);
    }
    out.into_bytes()
}

/// Срендерить один элемент с отступом `depth`, БЕЗ завершающего EOL.
fn render_element(env: &Envelope, el: &OutElement, depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str(env.indent_unit);
    }
    let qname = el.qname();
    out.push('<');
    out.push_str(&qname);
    for (name, value) in &el.attrs {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        out.push_str(&escape_with(value, env.escape_gt));
        out.push('"');
    }

    let has_children = !el.children.is_empty();
    let text = el.text.as_deref();

    if !has_children && text.is_none() {
        if el.self_closing {
            out.push_str("/>");
        } else {
            // Пустой не-самозакрывающийся: `<tag></tag>` (1С так пустой текст не
            // пишет — пустой comment самозакрывается; вариант оставлен для полноты).
            out.push_str("></");
            out.push_str(&qname);
            out.push('>');
        }
        return;
    }

    out.push('>');
    if has_children {
        // Ветка: дети с EOL и отступом, закрывающий тег на своей строке.
        for child in &el.children {
            out.push_str(env.eol);
            render_element(env, child, depth + 1, out);
        }
        out.push_str(env.eol);
        for _ in 0..depth {
            out.push_str(env.indent_unit);
        }
    } else if let Some(t) = text {
        // Лист: `<tag>text</tag>`. Канонический IR-текст хранит `\n`; разворачиваем in-
        // text перевод строки в конвенцию формата (EDT `\r\n` / Designer `\n`). Кавычка
        // экранируется per-format (`env.escape_quot`: EDT `&quot;`, Designer литерал).
        let escaped = escape_text_with(t, env.escape_gt, env.escape_quot);
        if env.text_eol == "\n" {
            out.push_str(&escaped);
        } else {
            out.push_str(&escaped.replace('\n', env.text_eol));
        }
    }
    out.push_str("</");
    out.push_str(&qname);
    out.push('>');
}
