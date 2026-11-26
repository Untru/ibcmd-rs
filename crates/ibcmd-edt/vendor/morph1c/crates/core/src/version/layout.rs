//! РАСКЛАДКИ сериализации по версиям формата как ДАННЫЕ.
//!
//! # Что это
//! `models/format_layouts.jsonl` отвечает на вопрос «какую ФОРМУ имеет конструкция в версии
//! формата V»: ведущее число записи, число слотов, арность, ростер пространств имён. Данные
//! сняты с ПЛАТФОРМЕННЫХ дампов ОДНОЙ И ТОЙ ЖЕ конфигурации, собранной разными платформами
//! (`scripts/version_probe.sh` + `scripts/version_surface_probe.sh` → `target/versions/` и
//! `target/version_surface/`), инструментом `tools/build_format_layouts.py`. То есть это
//! НАБЛЮДЕНИЕ, а не наша модель: если инструмент видит одну конструкцию РАЗНОЙ в пределах
//! версии, он отказывается писать факт.
//!
//! Здесь — только ДОСТУП: таблицу генерирует `build.rs`, рукописных «в 2.17 будет вот так» в
//! коде нет.
//!
//! # Зачем
//! Раньше форма конструкции выбиралась ветвлением по версии прямо в кодеке
//! (`if version >= 2.21 { … } else { … }`). Такое ветвление — данные, спрятанные в коде: их
//! нельзя ни проверить против дампа, ни дополнить новой версией, не правя кодек. Теперь кодек
//! спрашивает раскладку и берёт из неё ПАРАМЕТР.
//!
//! # Граница знания
//! Версия, для которой записи нет, — это НЕ «такая же, как соседняя». Ответ — `None`, и
//! вызывающий обязан отказать типизированной ошибкой ([`missing`] собирает для неё текст со
//! списком витнессированных версий). Единственное исключение — записи с `scope: "from"`:
//! самая новая витнессированная форма части конструкций распространяется и на более новые
//! версии. Это ДОСЛОВНО то, что делал код до перевода на данные (`version >= SSL`), и
//! перенесено в данные, чтобы перевод не поменял поведение; список таких конструкций живёт в
//! генераторе (`OPEN_ENDED`), а не здесь.

use super::FormatVersion;

/// Одна строка таблицы раскладок: форма КОНСТРУКЦИИ в конкретной версии формата.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Имя конструкции (`cf.form_body.envelope`) — ключ, общий с данными.
    pub construct: &'static str,
    /// Версия формата `(major, minor)`, в дампе которой форма наблюдалась.
    pub format: (u16, u16),
    /// Форма действует и для БОЛЕЕ НОВЫХ версий (см. модульный докстринг).
    pub open_ended: bool,
    /// Наблюдённые параметры формы `(имя, значение)`, отсортированы по имени.
    pub params: &'static [(&'static str, &'static str)],
    /// Ростер пространств имён `(имя/префикс, URI)` В ПОРЯДКЕ ДАМПА (пусто, если неприменимо).
    pub ns: &'static [(&'static str, &'static str)],
    /// Дамп, с которого форма снята (для сообщений об ошибке).
    pub witness: &'static str,
}

// Сгенерировано build.rs из `models/format_layouts.jsonl`: `FORMAT_LAYOUTS`.
include!(concat!(env!("OUT_DIR"), "/format_layout_tables.rs"));

impl Layout {
    /// Версия формата, в которой форма наблюдалась.
    pub fn version(&self) -> FormatVersion {
        FormatVersion::new(self.format.0, self.format.1)
    }

    /// Значение параметра, если он у этой раскладки есть.
    pub fn param(&self, name: &str) -> Option<&'static str> {
        self.params.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }

    /// Значение параметра как число. `None` — параметра нет либо он не число.
    pub fn param_usize(&self, name: &str) -> Option<usize> {
        self.param(name)?.parse().ok()
    }
}

/// Раскладка конструкции для целевой версии, если она ВИТНЕССИРОВАНА.
///
/// Правило выбора: берём САМУЮ НОВУЮ запись конструкции, чья версия не новее `target`; она
/// подходит, если это ровно `target` либо она открыта вперёд (`open_ended`). Иначе `None` —
/// версия не витнессирована, и подставлять соседнюю форму нельзя (§1.0).
pub fn layout(construct: &str, target: FormatVersion) -> Option<&'static Layout> {
    FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct && l.version() <= target)
        .max_by_key(|l| l.version())
        .filter(|l| l.open_ended || l.version() == target)
}

/// Раскладка конструкции РОВНО для этой версии (без распространения вперёд). Нужна там, где
/// прежний код сверял версию на равенство (детект версии по прочитанному дампу).
pub fn layout_exact(construct: &str, format: FormatVersion) -> Option<&'static Layout> {
    FORMAT_LAYOUTS
        .iter()
        .find(|l| l.construct == construct && l.version() == format)
}

/// Все раскладки конструкции (порядок таблицы; сортировать — забота вызывающего).
pub fn rows(construct: &str) -> Vec<&'static Layout> {
    FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct)
        .collect()
}

/// САМАЯ СТАРАЯ витнессированная раскладка конструкции.
///
/// Нужна ТОТАЛЬНЫМ (не возвращающим ошибку) местам кодеков: до них версия без раскладки не
/// доходит — корневая запись конфигурации отвергает её раньше, — а самая старая форма это
/// ровно то, что такие места выбирали прежним `если версия < 2.21 → легаси`.
pub fn oldest(construct: &str) -> Option<&'static Layout> {
    FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct)
        .min_by_key(|l| l.version())
}

/// НАИБОЛЬШЕЕ витнессированное значение числового параметра конструкции по всем версиям.
///
/// Нужна там, где место под ячейки резервируется один раз на все версии (самая длинная форма
/// задаёт ширину), — чтобы и эта ширина осталась ответом данных, а не числом в коде.
pub fn max_param_usize(construct: &str, name: &str) -> Option<usize> {
    FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct)
        .filter_map(|l| l.param_usize(name))
        .max()
}

/// Все витнессированные версии конструкции (по возрастанию).
pub fn versions(construct: &str) -> Vec<FormatVersion> {
    let mut v: Vec<FormatVersion> = FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct)
        .map(Layout::version)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Раскладка, у которой параметр `name` равен `value` — ОБРАТНЫЙ поиск «форма → версия».
///
/// Так читатель определяет версию дампа по тому, что в дампе написано (ведущее число
/// записи), не храня таблицу «число → версия» в коде. Если значение делят несколько версий,
/// возвращается САМАЯ СТАРАЯ из них: форма появилась там, а более новые её лишь унаследовали.
pub fn by_param(construct: &str, name: &str, value: &str) -> Option<&'static Layout> {
    FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct && l.param(name) == Some(value))
        .min_by_key(|l| l.version())
}

/// Все витнессированные значения параметра конструкции (по возрастанию версии) — для
/// сообщений об ошибке вида «известны: 76 → 2.21, 68 → 2.20».
pub fn param_witnesses(construct: &str, name: &str) -> Vec<(&'static str, FormatVersion)> {
    let mut rows: Vec<&'static Layout> = FORMAT_LAYOUTS
        .iter()
        .filter(|l| l.construct == construct)
        .collect();
    rows.sort_by_key(|l| l.version());
    rows.iter()
        .filter_map(|l| l.param(name).map(|v| (v, l.version())))
        .collect()
}

/// Текст типизированного отказа «раскладка конструкции для версии не витнессирована».
///
/// Собирается ЕДИНООБРАЗНО из данных, поэтому список известных версий в сообщении не
/// разъезжается с таблицей.
pub fn missing(construct: &str, target: FormatVersion) -> String {
    let known = versions(construct);
    let known = if known.is_empty() {
        "ни одной".to_string()
    } else {
        known
            .iter()
            .map(FormatVersion::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "раскладка `{construct}` для версии формата {target} не витнессирована \
         (витнессированы: {known}) — отказываюсь писать угаданную форму (§1.0). \
         Сначала получите дамп этой версии (scripts/version_probe.sh) и пересоберите \
         models/format_layouts.jsonl (tools/build_format_layouts.py)."
    )
}

/// Текст типизированного отказа «параметра нет у витнессированной раскладки». Возникает при
/// рассинхроне данных и кода (данные обновились, параметр переименован).
pub fn missing_param(construct: &str, target: FormatVersion, name: &str) -> String {
    format!(
        "раскладка `{construct}` версии {target} не несёт параметра `{name}` — \
         данные (models/format_layouts.jsonl) и код разошлись; пересоберите данные \
         инструментом tools/build_format_layouts.py"
    )
}

#[cfg(any())]
mod tests {
    use super::*;

    const V217: FormatVersion = FormatVersion::new(2, 17);
    const V220: FormatVersion = FormatVersion::new(2, 20);
    const V221: FormatVersion = FormatVersion::new(2, 21);

    /// Анти-вакуумность: таблица непуста и несёт ось версий, ради которой заведена.
    #[test]
    fn layout_table_is_populated() {
        assert!(FORMAT_LAYOUTS.len() >= 20, "строк: {}", FORMAT_LAYOUTS.len());
        assert_eq!(
            versions("cf.configuration_root.record"),
            vec![V217, V220, V221]
        );
    }

    /// Форма достаётся ПО ВЕРСИИ, а не одна на все.
    #[test]
    fn layout_is_a_function_of_version() {
        let get = |v| {
            layout("cf.configuration_root.record", v)
                .and_then(|l| l.param("lead"))
                .unwrap()
        };
        assert_eq!(get(V217), "67");
        assert_eq!(get(V220), "68");
        assert_eq!(get(V221), "76");
    }

    /// Невитнессированная версия — ОТКАЗ (`None`), а не форма соседней версии.
    #[test]
    fn unwitnessed_version_has_no_layout() {
        assert!(layout("cf.configuration_root.record", FormatVersion::new(2, 19)).is_none());
        assert!(layout("cf.configuration_root.record", FormatVersion::new(2, 13)).is_none());
        // …а самая новая форма открыта вперёд — ровно как было в коде (`>= SSL`).
        assert_eq!(
            layout("cf.configuration_root.record", FormatVersion::new(2, 22))
                .and_then(|l| l.param("lead")),
            Some("76")
        );
        // Конструкция, которой в данных нет вовсе, — тоже отказ.
        assert!(layout("cf.no_such_construct", V221).is_none());
    }

    /// Обратный поиск «ведущее число записи → версия»: это вход ЧТЕНИЯ.
    #[test]
    fn reverse_lookup_by_param() {
        let f = |lead| {
            by_param("cf.configuration_root.record", "lead", lead).map(|l| l.version())
        };
        assert_eq!(f("67"), Some(V217));
        assert_eq!(f("68"), Some(V220));
        assert_eq!(f("76"), Some(V221));
        assert_eq!(f("99"), None);
    }

    /// Ростер ns — тоже данные версии (дельта 2.21 — объявление `pal`).
    #[test]
    fn ns_roster_is_versioned() {
        let ns = |v| layout("designer.descriptor.envelope", v).unwrap().ns;
        assert_eq!(ns(V220).len() + 1, ns(V221).len());
        assert!(!ns(V220).iter().any(|(n, _)| *n == "xmlns:pal"));
        assert!(ns(V221).iter().any(|(n, _)| *n == "xmlns:pal"));
        // XML-обёртки НЕ распространяются вперёд — как и было у прежнего реестра профилей.
        assert!(layout("designer.descriptor.envelope", FormatVersion::new(2, 22)).is_none());
    }
}
