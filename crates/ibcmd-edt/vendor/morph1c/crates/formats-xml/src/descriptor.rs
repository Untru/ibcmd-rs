//! Разобранный XML-дескриптор объекта как структурное дерево — носитель `Source`
//! движка и предмет учёта тотальности (§1.0).
//!
//! Дерево намеренно МИНИМАЛЬНО: только то, что несёт дескриптор объекта 1С —
//! элементы (имя + опц. ns-префикс + атрибуты + дети + текст). Узлы помечаются
//! «востребованными» по мере того как проекция читает поля; `unclaimed_count`
//! считает невостребованное → движок требует 0 (нет passthrough/Raw).

use std::cell::Cell;

/// Атрибут элемента (`name="value"`), значение уже UNescape'нуто.
#[derive(Debug, Clone)]
pub struct Attr {
    /// Имя атрибута как в источнике (включая возможный префикс `xmlns:` и т.п.).
    pub name: String,
    /// Декодированное (unescaped) значение.
    pub value: String,
    /// Востребован ли атрибут проекцией/каркасом (для тотальности).
    pub claimed: Cell<bool>,
}

/// Элемент дерева дескриптора.
#[derive(Debug, Clone)]
pub struct Element {
    /// Local-name тега (без префикса).
    pub local: String,
    /// Префикс ns (`""` = без префикса). Для EDT-корня — `mdclass`.
    pub prefix: String,
    /// Атрибуты в исходном порядке.
    pub attrs: Vec<Attr>,
    /// Дочерние элементы в исходном порядке.
    pub children: Vec<Element>,
    /// Непосредственный текст элемента (unescaped, БЕЗ чисто-пробельных кусков —
    /// форматирование отброшено токенайзером). Для листьев — значение; для
    /// контейнеров обычно пуст. Непустой текст ОБЯЗАН быть востребован кодеком
    /// (`text_claimed`), иначе он — несконсуменный фрагмент (§1.0).
    pub text: String,
    /// Был ли САМ элемент-узел востребован проекцией/каркасом (для тотальности §1.0).
    pub claimed: Cell<bool>,
    /// Был ли НЕПОСРЕДСТВЕННЫЙ ТЕКСТ элемента востребован кодеком. Отдельно от
    /// `claimed`: контейнер/presence-узел claim'ит сам элемент, но НЕ произвольный
    /// текст — иначе мусорный текст между детьми (B1) проглатывался бы молча.
    pub text_claimed: Cell<bool>,
}

impl Element {
    /// Новый пустой элемент.
    pub fn new(prefix: impl Into<String>, local: impl Into<String>) -> Self {
        Element {
            local: local.into(),
            prefix: prefix.into(),
            attrs: Vec::new(),
            children: Vec::new(),
            text: String::new(),
            claimed: Cell::new(false),
            text_claimed: Cell::new(false),
        }
    }

    /// Найти ПЕРВЫЙ непосредственный дочерний элемент с данным local-name.
    pub fn child(&self, local: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.local == local)
    }

    /// Найти атрибут по имени.
    pub fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.name == name)
    }

    /// Пометить САМ элемент-узел востребованным (НЕ его текст — текст клеймит кодек,
    /// читающий значение, через [`claim_text`](Self::claim_text)).
    pub fn claim(&self) {
        self.claimed.set(true);
    }

    /// Пометить непосредственный ТЕКСТ элемента востребованным (кодек прочитал
    /// значение из текста — PlainText/EnumText/BoolPresence/key/value).
    pub fn claim_text(&self) {
        self.text_claimed.set(true);
    }

    /// Пометить элемент И его текст (удобный хелпер для текст-несущих листьев).
    pub fn claim_with_text(&self) {
        self.claim();
        self.claim_text();
    }

    /// Рекурсивно пометить поддерево (элемент + текст + все дети + их атрибуты).
    pub fn claim_subtree(&self) {
        self.claim();
        self.claim_text();
        for a in &self.attrs {
            a.claimed.set(true);
        }
        for c in &self.children {
            c.claim_subtree();
        }
    }

    /// Сколько узлов в поддереве (элемент + его непустой невостребованный текст +
    /// атрибуты + дети рекурсивно) ещё НЕ востребованы. Движок требует 0 от корня
    /// объекта (§1.0). Непустой текст без `text_claimed` считается ОТДЕЛЬНЫМ
    /// несконсуменным узлом — это и закрывает дыру B1 (мусорный текст между детьми).
    pub fn unclaimed_count(&self) -> usize {
        let mut n = 0;
        if !self.claimed.get() {
            n += 1;
        }
        // Непустой текст (whitespace уже отброшен на чтении) обязан быть востребован.
        if !self.text.is_empty() && !self.text_claimed.get() {
            n += 1;
        }
        for a in &self.attrs {
            if !a.claimed.get() {
                n += 1;
            }
        }
        for c in &self.children {
            n += c.unclaimed_count();
        }
        n
    }

    /// Имена (пути) НЕвостребованных УЗЛОВ поддерева — для диагностики `UnconsumedInput`
    /// (какие именно ячейки спек не покрыл). Возвращает до `limit` имён: элемент —
    /// `parent/child`, атрибут — `parent/@name`, текст — `parent/#text` (leftover бывает и
    /// атрибутом/текстом при заклеймленном элементе — прежняя версия их не перечисляла, и
    /// диагностика показывала «N cells» без имён). Пусто ⇒ всё поддерево востребовано.
    pub fn unclaimed_names(&self, limit: usize) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_unclaimed(String::new(), limit, &mut out);
        out
    }

    fn collect_unclaimed(&self, prefix: String, limit: usize, out: &mut Vec<String>) {
        if out.len() >= limit {
            return;
        }
        for c in &self.children {
            let path = if prefix.is_empty() {
                c.local.clone()
            } else {
                format!("{prefix}/{}", c.local)
            };
            // Незаклеймленный элемент называем сам (его поддерево не перечисляем — имя
            // корня leftover-региона диагностически достаточно); под заклеймленным
            // рекурсим — там могут прятаться невостребованные атрибуты/текст.
            if !c.claimed.get() && out.len() < limit {
                out.push(path.clone());
            } else {
                c.collect_unclaimed(path, limit, out);
            }
        }
        for a in &self.attrs {
            if !a.claimed.get() && out.len() < limit {
                out.push(if prefix.is_empty() {
                    format!("@{}", a.name)
                } else {
                    format!("{prefix}/@{}", a.name)
                });
            }
        }
        if !self.text.is_empty() && !self.text_claimed.get() && out.len() < limit {
            out.push(if prefix.is_empty() {
                "#text".to_string()
            } else {
                format!("{prefix}/#text")
            });
        }
    }
}
