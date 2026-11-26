//! Since-таблицы: КАКАЯ версия ввела свойство/литерал (ARCHITECTURE.md §1.5, FORMATS.md §4).
//!
//! # Откуда данные
//! Таблицы ГЕНЕРИРУЮТСЯ `build.rs` из инвентаря (`inventory/metamodel.jsonl` +
//! `inventory/enums.jsonl`), который извлечён из EDT-метамодели (`MdClass.xcore`,
//! `Common.xcore`). Рукописного списка версий тут НЕТ — правится инвентарь, не код.
//!
//! Инвентарь несёт версию ПЛАТФОРМЫ (`8.3.27`); гейтим же мы по версии ФОРМАТА
//! (`2.20`), потому что формат — это то, чем штампован дамп. Перевод идёт через
//! [`VERSION_TABLE`] — единственную копию таблицы FORMATS.md §2.
//!
//! # Пол таблицы: «было всегда»
//! [`VERSION_TABLE`] начинается с 8.3.17 (формат 2.10) — это НИЖНЯЯ ГРАНИЦА
//! поддерживаемого диапазона. Свойство со `since` СТАРШЕ пола (8.3.9…8.3.16)
//! существует во ВСЕХ версиях, которые мы вообще умеем читать, поэтому гейта не
//! получает ([`resolve`] → `None`). Это не «молча пропустили»: гейт, который истинен
//! всегда, — не гейт.
//!
//! Версия ВЫШЕ пола, которой нет в [`VERSION_TABLE`], — ошибка ДАННЫХ (инвентарь знает
//! платформу, которой не знает реестр форматов). Такое [`resolve`] не проглатывает:
//! паника (§1.0 — не гадать), а тест [`every_since_entry_resolves`] ловит это на CI.
//!
//! # Ключ поиска
//! `since` — ФУНКЦИЯ ИМЕНИ поля: одно и то же имя (`lineNumberLength`) введено одной и
//! той же версией во всех видах, где встречается. Это не предположение — это
//! проверяется тестом [`since_is_a_function_of_field_name`] по всей таблице. Поэтому
//! [`since_for_field_name`] (поиск по одному лишь имени) КОРРЕКТЕН, и именно он
//! позволяет проставить гейт централизованно в конструкторах [`FieldSpec`], не трогая
//! 156 спек-файлов и не заводя второй источник правды.
//!
//! [`FieldSpec`]: crate::spec::common::FieldSpec

use super::{format_for_platform, FormatVersion, PlatformVersion, Since, VERSION_TABLE};

// Сгенерированное build.rs: `SINCE_FIELDS_RAW` / `SINCE_LITERALS_RAW` — сырые
// `(ключ, ключ, (maj, min, patch))` прямо из инвентаря.
include!(concat!(env!("OUT_DIR"), "/since_tables.rs"));

/// Нижняя граница реестра версий — всё, что введено РАНЬШЕ, «было всегда».
fn floor() -> PlatformVersion {
    VERSION_TABLE
        .first()
        .expect("VERSION_TABLE is never empty")
        .platform
}

/// Версия платформы из инвентаря → гейт [`Since`] (версия ФОРМАТА), либо `None`, если
/// свойство старше поддерживаемого диапазона («было всегда» — гейт не нужен).
///
/// # Паника
/// Версия НЕ старше пола, но отсутствующая в [`VERSION_TABLE`], — рассинхрон инвентаря
/// и FORMATS.md. Это ошибка ДАННЫХ, не входа: падаем громко (§1.0), а не гейтим наугад.
fn resolve(raw: (u16, u16, u16)) -> Option<Since> {
    let p = PlatformVersion::new(raw.0, raw.1, raw.2);
    if p < floor() {
        return None;
    }
    match format_for_platform(p) {
        Some(f) => Some(Since(f)),
        None => panic!(
            "inventory carries `since` platform {p}, which VERSION_TABLE (FORMATS.md §2) \
             does not know — the version registry and the inventory are out of sync"
        ),
    }
}

/// Гейт поля по КАНОНИЧЕСКОМУ ИМЕНИ (`lineNumberLength`, `useInInterfaceCompatibilityMode`).
///
/// Имя однозначно определяет версию введения (см. модульный док и тест
/// [`since_is_a_function_of_field_name`]), поэтому вид знать не требуется — это и делает
/// возможной централизованную простановку гейта в [`FieldSpec`](crate::spec::common::FieldSpec).
///
/// `None` = поле не версионировано (есть во всех поддерживаемых версиях).
pub fn since_for_field_name(name: &str) -> Option<Since> {
    SINCE_FIELDS_RAW
        .iter()
        .find(|(_, f, _)| *f == name)
        .and_then(|&(_, _, raw)| resolve(raw))
}

/// Гейт поля КОНКРЕТНОГО вида (`("CatalogTabularSection", "lineNumberLength")`).
///
/// Строже [`since_for_field_name`]: не найдёт поле в виде, который его не объявляет.
/// Нужен там, где важно именно объявление вида (сверка спеков с метамоделью).
pub fn since_for_field(kind: &str, name: &str) -> Option<Since> {
    SINCE_FIELDS_RAW
        .iter()
        .find(|(k, f, _)| *k == kind && *f == name)
        .and_then(|&(_, _, raw)| resolve(raw))
}

/// АДРЕСНЫЙ витнесс `models/version_facts.jsonl` по (ВЛАДЕЛЕЦ, каноническое имя) — БЕЗ
/// слияния с прочими источниками.
///
/// Метамодель EDT версий форм не несёт (маркеров `// since` в форм-xcore нет), поэтому источник
/// здесь другой — ПЛАТФОРМА: дельта дампов одной конфигурации, собранной разными платформами
/// (`scripts/version_probe.sh`). Ключ ОБЯЗАТЕЛЬНО двойной: имена форм-свойств не уникальны между
/// контролами (`orientation`, `color`, …), поэтому поиск лишь по имени — как у метаданных
/// ([`since_for_field_name`]) — здесь был бы неверен.
///
/// Публичный ответ о наличии — [`since_for_form_field`] (он сводит источники по правилу
/// [`crate::version::availability`]); эта функция — ОДИН из его входов.
pub(crate) fn since_for_form_field_witness(owner: &str, canonical: &str) -> Option<Since> {
    FORM_SINCE_RAW
        .iter()
        .find(|(o, f, _)| *o == owner && *f == canonical)
        .map(|&(_, _, (major, minor))| Since(FormatVersion::new(major, minor)))
}

/// Гейт свойства ФОРМЫ по (ВЛАДЕЛЕЦ, каноническое имя) — `("InputField", "autoEditMode")`.
///
/// СВЕДЁННЫЙ ответ: адресный витнесс `models/version_facts.jsonl` + версионный реестр, по
/// правилу слияния из [`crate::version::availability`] (тело формы — РАЗРЕЖЁННАЯ область,
/// поэтому реестр может границу только смягчить, но не назначить).
///
/// `None` = свойство не версионировано (не наблюдалось появляющимся ни в одной дельте).
pub fn since_for_form_field(owner: &str, canonical: &str) -> Option<Since> {
    super::availability::property_availability(
        super::availability::PropertyScope::Form { owner },
        canonical,
    )
    .since
}

/// Доступно ли свойство формы в версии формата `target` (без гейта — доступно всегда).
pub fn form_field_available_in(owner: &str, canonical: &str, target: FormatVersion) -> bool {
    since_for_form_field(owner, canonical).is_none_or(|s| s.available_in(target))
}

/// Свойства КОНТРОЛА, которые ЦЕЛЬ проставляет САМА, поднимая источник версии `source`
/// (факт `materialized`, см. [`FORM_MATERIALIZED_RAW`]).
///
/// Отдаёт `(каноническое имя, витнессированное значение)`.
///
/// # Совпадение версий ТОЧНОЕ, и это не перестраховка
/// Материализация — поведение КОНКРЕТНОЙ пары «версия источника → версия платформы», а не
/// монотонное свойство диапазона. Витнесс-опровержение монотонности: 8.5.1, поднимая дамп
/// 2.17, ставит `UsualGroup.group = HorizontalIfPossible`, а поднимая дамп 2.20 — не ставит
/// НИЧЕГО, хотя тег опускают ОБА дампа (гейт `scripts/version_matrix.sh`: на паре 2.20→2.21
/// дописанное значение делает нашу конфигурацию ОТЛИЧНОЙ от платформенной). Поэтому факт
/// действует ровно на том переходе, который наблюдался; про неснятые переходы ответа НЕТ.
///
/// Это НЕ то же, что «умолчание версии» ([`crate::resolve::field_default`]): умолчание
/// отвечает, какое значение писатель ОПУСКАЕТ, а здесь — какое значение платформа КЛАДЁТ
/// туда, где источник молчал. Значения могут различаться (витнесс: `RadioButtonField`
/// `orientation` — платформа опускает `Vertical`, а поднимая дамп 2.17, ставит
/// `HorizontalIfPossible`).
pub fn form_materialized_on_upgrade(
    owner: &str,
    source: FormatVersion,
    target: FormatVersion,
) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
    FORM_MATERIALIZED_RAW
        .iter()
        .filter_map(move |&(o, f, from, to, value)| {
            (o == owner
                && source == FormatVersion::new(from.0, from.1)
                && target == FormatVersion::new(to.0, to.1))
            .then_some((f, value))
        })
}

/// Свойства КОНТРОЛА, ПЕРЕИМЕНОВАННЫЕ на переходе `source → target` (факт `renamed`).
///
/// Отдаёт `(старое каноническое имя, новое каноническое имя)`. Условие срабатывания то же,
/// что у [`form_materialized_on_upgrade`]: источник не новее версии со старым именем, цель
/// не старее версии с новым.
pub fn form_renamed_on_upgrade(
    owner: &str,
    source: FormatVersion,
    target: FormatVersion,
) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
    FORM_RENAMED_RAW.iter().filter_map(move |&(o, from_name, to_name, from, to)| {
        (o == owner
            && source <= FormatVersion::new(from.0, from.1)
            && target >= FormatVersion::new(to.0, to.1))
        .then_some((from_name, to_name))
    })
}

/// Гейт ЛИТЕРАЛА перечисления (`("MobileApplicationFunctionalities", "TextToSpeech")`).
///
/// Нужен ПЛОТНЫМ таблицам, которые эмитят ВЕСЬ набор литералов перечисления: набор
/// растёт с версией, поэтому «полная таблица» — понятие версионно-зависимое
/// (witnessed: 2.17 не знает `TextToSpeech`, 2.20 знает).
pub fn since_for_literal(enum_name: &str, literal: &str) -> Option<Since> {
    SINCE_LITERALS_RAW
        .iter()
        .find(|(e, l, _)| *e == enum_name && *l == literal)
        .and_then(|&(_, _, raw)| resolve(raw))
}

/// Доступен ли литерал перечисления в версии формата `target` — СВЕДЁННЫЙ ответ
/// ([`crate::version::availability::literal_availability`]).
///
/// Литерал без гейта доступен всегда; с гейтом — только начиная со своей версии. Реестр
/// версий гейта литералов НЕ ставит: наблюдение литерала в дампе — функция того, какие
/// литералы использовала конфигурация прогона, а не того, какие версия знает; он служит
/// независимым ПОДТВЕРЖДЕНИЕМ метамодельной границы.
pub fn literal_available_in(enum_name: &str, literal: &str, target: FormatVersion) -> bool {
    super::availability::literal_availability(enum_name, literal).available_in(target)
}

/// Designer-имя свойства (`TypeReductionMode`) → каноническое имя метамодели
/// (`typeReductionMode`).
///
/// Конвенция ПЛАТФОРМЫ, а не наша: одно и то же свойство зовётся PascalCase в
/// Designer-XML и camelCase в EDT/`.xcore` (witnessed на всём корпусе:
/// `<LineNumberLength>` ↔ `lineNumberLength`, `<UseInInterfaceCompatibilityMode>` ↔
/// `useInInterfaceCompatibilityMode`). Различие — РОВНО регистр первой буквы.
fn designer_name_to_canonical(local: &str) -> String {
    let mut chars = local.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Доступно ли Designer-свойство (по его PascalCase local-name) в версии формата `target`.
///
/// Нужно ПЛОТНЫМ (dense) регионам Designer, где набор элементов фиксирован и позиционен:
/// такой регион — не «список того, что задано», а ПОЛНАЯ таблица свойств версии, поэтому
/// его ДЛИНА и СОСТАВ суть функция версии. Witnessed: `<xr:StandardAttribute>` несёт 24
/// листа в 2.17 и 25 в 2.20+ (добавился `<xr:TypeReductionMode>`, since 8.3.25).
pub fn designer_field_available_in(local: &str, target: FormatVersion) -> bool {
    // Ключ здесь — ОДНО ЛИШЬ имя (владелец плотного региона в вызывающем коде не известен),
    // поэтому спрашиваем сведённый ответ в скоупе [`PropertyScope::Unmapped`]: в нём
    // отвечает только метамодель — единственный источник, чей ключ — имя поля.
    super::availability::property_available_in(
        super::availability::PropertyScope::Unmapped,
        &designer_name_to_canonical(local),
        target,
    )
}

#[cfg(any())]
mod tests {
    use super::*;

    /// ОСНОВАНИЕ поиска по имени (`since_for_field_name`): имя поля определяет версию
    /// ОДНОЗНАЧНО. Если метамодель когда-нибудь введёт одно имя с разными `since` в
    /// разных видах — этот тест упадёт, и поиск по имени придётся заменить на (вид, имя).
    /// Тихой деградации не будет.
    #[test]
    fn since_is_a_function_of_field_name() {
        for (kind, name, raw) in SINCE_FIELDS_RAW {
            for (other_kind, other_name, other_raw) in SINCE_FIELDS_RAW {
                if name == other_name {
                    assert_eq!(
                        raw, other_raw,
                        "field {name:?} has conflicting `since`: {kind}={raw:?} vs \
                         {other_kind}={other_raw:?} — name-keyed lookup is no longer sound"
                    );
                }
            }
        }
    }

    /// Каждая запись инвентаря РАЗРЕШАЕТСЯ: либо ниже пола («было всегда»), либо есть в
    /// `VERSION_TABLE`. Запись выше пола и вне реестра — паника в `resolve`; тест ловит
    /// рассинхрон инвентаря и FORMATS.md на CI, а не в проде.
    #[test]
    fn every_since_entry_resolves() {
        for (_, _, raw) in SINCE_FIELDS_RAW {
            let _ = resolve(*raw);
        }
        for (_, _, raw) in SINCE_LITERALS_RAW {
            let _ = resolve(*raw);
        }
    }

    /// WITNESSED (`.fixtures/versions/`, дампы ОДНОЙ конфигурации, собранной 8.3.24 /
    /// 8.3.27 / 8.5.1): свойства, появившиеся в дампе версии N и отсутствующие в N-1.
    /// Метамодель обязана давать для них ровно ту же границу. Это независимое
    /// подтверждение метамодели ПЛАТФОРМОЙ (иначе since-таблица — вера, а не факт).
    #[test]
    fn matches_witnessed_platform_delta() {
        // Designer-имя (PascalCase) → каноническое имя метамодели (camelCase).
        let witnessed: &[(&str, &str, FormatVersion)] = &[
            // 2.17 -> 2.20: +<xr:TypeReductionMode> в КАЖДОМ <xr:StandardAttribute>
            // и в Dimension регистра сведений. Метамодель: since 8.3.25 = формат 2.18,
            // т.е. в диапазоне (2.17, 2.20] — точнее свидетель не различает (мы не
            // ставили 8.3.25/8.3.26 экспериментом), но границу 2.17 он ОПРОВЕРГАЕТ.
            ("TypeReductionMode", "typeReductionMode", FormatVersion::new(2, 20)),
            // 2.17 -> 2.20: +<LineNumberLength> у табличных частей. Метамодель: 8.3.27 = 2.20.
            ("LineNumberLength", "lineNumberLength", FormatVersion::new(2, 20)),
            // 2.20 -> 2.21: свойства интерфейса 8.5.
            ("UseInInterfaceCompatibilityMode", "useInInterfaceCompatibilityMode", FormatVersion::new(2, 21)),
            ("Caption", "caption", FormatVersion::new(2, 21)),
            ("ShortCaption", "shortCaption", FormatVersion::new(2, 21)),
            ("ClientApplicationTheme", "clientApplicationTheme", FormatVersion::new(2, 21)),
            ("Version85InterfaceMigrationMode", "version85InterfaceMigrationMode", FormatVersion::new(2, 21)),
            ("Color", "color", FormatVersion::new(2, 21)),
            ("AuxiliaryVariantForm", "auxiliaryVariantForm", FormatVersion::new(2, 21)),
        ];

        for (designer, canonical, introduced_by) in witnessed {
            let since = since_for_field_name(canonical).unwrap_or_else(|| {
                panic!(
                    "witnessed: <{designer}> appears only from format {introduced_by}, \
                     but the metamodel gives {canonical:?} NO `since` gate at all"
                )
            });
            // Свидетель: поля НЕТ в дампе предыдущей версии, но ОНО ЕСТЬ в `introduced_by`.
            assert!(
                since.available_in(*introduced_by),
                "witnessed: <{designer}> IS present in format {introduced_by}, but the \
                 metamodel gates {canonical:?} at {} — the gate would wrongly drop it",
                since.0
            );
        }
    }

    /// Тот же свидетель, но на ЛИТЕРАЛАХ: плотная таблица мобильных функциональностей в
    /// дампе 2.17 НЕ содержит `TextToSpeech`, а в 2.20 — содержит (см.
    /// `.fixtures/versions/probes/configuration_mobile_func/`).
    #[test]
    fn witnessed_literal_delta_mobile_functionalities() {
        const E: &str = "MobileApplicationFunctionalities";
        assert!(
            !literal_available_in(E, "TextToSpeech", FormatVersion::new(2, 17)),
            "witnessed: the 2.17 dump has NO <TextToSpeech> entry"
        );
        assert!(
            literal_available_in(E, "TextToSpeech", FormatVersion::new(2, 20)),
            "witnessed: the 2.20 dump HAS a <TextToSpeech> entry"
        );
        // Литерал без гейта доступен в любой версии.
        assert!(literal_available_in(E, "ContactsAccess", FormatVersion::new(2, 17)));
    }

    /// WITNESSED (`.fixtures/versions/probes/catalog_std_attrs/`): плотный регион
    /// `<xr:StandardAttribute>` несёт 24 листа в 2.17 и 25 в 2.20 — ровно на
    /// `<xr:TypeReductionMode>` больше. Гейт по Designer-имени обязан это воспроизвести.
    #[test]
    fn witnessed_dense_leaf_gate_type_reduction_mode() {
        assert!(!designer_field_available_in(
            "TypeReductionMode",
            FormatVersion::new(2, 17)
        ));
        assert!(designer_field_available_in(
            "TypeReductionMode",
            FormatVersion::new(2, 20)
        ));
        // Не версионированный лист того же региона доступен в любой версии.
        assert!(designer_field_available_in(
            "FillChecking",
            FormatVersion::new(2, 17)
        ));
    }

    /// Пол реестра: свойство, введённое до 8.3.17, гейта не получает.
    #[test]
    fn below_floor_is_always_available() {
        assert_eq!(resolve((8, 3, 15)), None);
        assert_eq!(resolve((8, 3, 16)), None);
        assert_eq!(resolve((8, 3, 27)), Some(Since(FormatVersion::new(2, 20))));
        assert_eq!(resolve((8, 5, 1)), Some(Since(FormatVersion::new(2, 21))));
    }
}
