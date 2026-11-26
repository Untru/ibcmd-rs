//! Downgrade-guard: отказ ПЕРЕД записью, если IR несёт свойства новее ТАРГЕТ-версии
//! (ARCHITECTURE.md §1.0 «не знаешь — падай», §1.5).
//!
//! # Зачем guard, а не тихий гейт
//! Движок ([`crate::engine::write`]) НЕ ЭМИТИТ поле, чьё `since` новее таргета, — иначе
//! он выдал бы дамп, который платформа не прочитает. Но «не эмитить ПРИСУТСТВУЮЩЕЕ
//! значение» — это ПОТЕРЯ ДАННЫХ. Молча она недопустима (§1.0). Поэтому запись всегда
//! предваряется этим проходом: он находит ВСЕ такие свойства и падает СПИСКОМ.
//!
//! Обратное — свойство новее таргета, но равное дефолту — потерей НЕ является: его и не
//! было в источнике, оно лишь восстановлено спеком. Такое не репортится (иначе гейт
//! ругался бы на каждую конверсию вниз).
//!
//! # Оракул
//! Платформа отказывает симметрично, только грубее — по ВЕРСИИ ОБЁРТКИ, не доходя до
//! свойств (witnessed, 8.3.27 на дампе SSL 2.21):
//! ```text
//! Неизвестная версия формата 2.21 загружаемого файла …/Configuration.xml
//! ```
//! Мы читаем 2.21 и отказываемся ПИСАТЬ 2.20 — с точным перечнем виновных свойств, т.е.
//! строго информативнее платформы, и так же громко.

use crate::ir::{Configuration, MetadataObject};
use crate::spec::registry::spec_for;
use crate::version::FormatVersion;

/// Одно нарушение: свойство, ПРИСУТСТВУЮЩЕЕ в IR и введённое ПОЗЖЕ таргета.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Канонический код вида (`"Catalog"`), либо `"Configuration"` для корня.
    pub kind: String,
    /// Имя объекта (пусто у корня конфигурации).
    pub object: String,
    /// Каноническое имя свойства (`"useInInterfaceCompatibilityMode"`).
    pub field: &'static str,
    /// Версия формата, ВВЕДШАЯ свойство.
    pub since: FormatVersion,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.object.is_empty() {
            write!(f, "{}.{} (since {})", self.kind, self.field, self.since)
        } else {
            write!(
                f,
                "{}.{}.{} (since {})",
                self.kind, self.object, self.field, self.since
            )
        }
    }
}

/// Ошибка downgrade'а: таргет старее, чем свойства, которые НЕСЁТ конфигурация.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DowngradeError {
    /// Запрошенная целевая версия формата.
    pub target: FormatVersion,
    /// ВСЕ нарушения (не первое попавшееся — весь список, чтобы чинить за один заход).
    pub violations: Vec<Violation>,
}

impl std::fmt::Display for DowngradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "refusing to write format {}: the configuration carries {} propert{} introduced \
             by a LATER format — emitting the target would silently drop {} (§1.0)",
            self.target,
            self.violations.len(),
            if self.violations.len() == 1 {
                "y"
            } else {
                "ies"
            },
            if self.violations.len() == 1 {
                "it"
            } else {
                "them"
            },
        )?;
        for v in &self.violations {
            writeln!(f, "  {v}")?;
        }
        write!(
            f,
            "use a --v8version whose format is at least {}",
            self.violations
                .iter()
                .map(|v| v.since)
                .max()
                .unwrap_or(self.target)
        )
    }
}

impl std::error::Error for DowngradeError {}

/// Проверить конфигурацию против ТАРГЕТ-версии. `Ok(())` — писать безопасно.
///
/// Собирает нарушения по ВСЕЙ конфигурации (корень + объекты + вложенные child-объекты),
/// а не падает на первом: пользователю нужен полный список, а не итеративная игра.
///
/// ВЕРСИЮ ИСТОЧНИКА гейт берёт ИЗ САМОГО IR ([`Configuration::source_version`]) — отдельным
/// параметром её протаскивать больше не нужно (см. [`retain_present_in_source`]).
pub fn check_downgrade(
    cfg: &Configuration,
    target: FormatVersion,
) -> Result<(), Box<DowngradeError>> {
    let mut violations = Vec::new();
    collect(
        "Configuration",
        "",
        &cfg.properties,
        target,
        &mut violations,
    );
    for obj in &cfg.objects {
        walk_object(obj, target, &mut violations);
    }
    retain_present_in_source(&mut violations, cfg.source_version);
    downgrade_result(target, violations)
}

/// Отсеять нарушения, которые ИСТОЧНИК нести не мог, — страховка от ЛОЖНЫХ тревог.
///
/// # Зачем версия источника
/// Guard судит по IR, а IR — реконструкция: часть значений ридер восстанавливает по конвенции
/// диалекта (омиссия тега = дефолт формата). Для поля БЕЗ умолчания отличить «пришло из
/// файла» от «достроено ридером» по одному лишь мешку нельзя.
///
/// Отличает ВЕРСИЯ: свойство, введённое версией V, физически не могло прийти из дампа версии
/// СТАРШЕ V — там его не существует, значит и потерять его нельзя. Witnessed-случай:
/// 2.17-дамп не несёт `<Orientation>` у RadioButtonField (свойство введено 2.21).
///
/// Ридеры такие свойства больше и не материализуют (версионный гейт реконструкции,
/// `core::version::with_source_version`), так что на XML-лейнах отсеивать обычно уже нечего.
/// Проход остаётся ВТОРЫМ рубежом: он покрывает ридеры, до которых версия источника не
/// доходит (синтетический IR, самостоятельные тела-сайдкары), и стоит один проход по списку.
///
/// `source == None` — версия входа неизвестна: не отсеиваем ничего (судим по одному IR).
pub fn retain_present_in_source(violations: &mut Vec<Violation>, source: Option<FormatVersion>) {
    if let Some(src) = source {
        violations.retain(|v| v.since <= src);
    }
}

/// Накопить нарушения ОДНОГО объекта (и его child-объектов) в `out`.
///
/// Для вызывающего, который не держит `Configuration` целиком: `.cf`-лейн читает объекты
/// ПОТОКОМ, кодирует и освобождает каждый (`pipeline::cf_stream`), поэтому проверяет их по
/// одному и сводит накопленное [`downgrade_result`]'ом. Порядок нарушений в итоговом списке —
/// на вызывающем (он и решает, в каком порядке обходит объекты); [`check_downgrade`] обходит
/// их в порядке `cfg.objects`.
pub fn collect_downgrade_violations(
    obj: &MetadataObject,
    target: FormatVersion,
    out: &mut Vec<Violation>,
) {
    walk_object(obj, target, out);
}

/// [`collect_downgrade_violations`] для СВОЙСТВ КОНФИГУРАЦИИ (`cfg.properties`, вид
/// `Configuration`, объект — пустое имя).
pub fn collect_config_downgrade_violations(
    props: &[(crate::ir::FieldId, crate::ir::value::PropertyValue)],
    target: FormatVersion,
    out: &mut Vec<Violation>,
) {
    collect("Configuration", "", props, target, out);
}

/// Свернуть накопленные нарушения в результат: пусто → `Ok(())`, иначе — [`DowngradeError`]
/// со ВСЕМ списком (та же ошибка, что вернул бы [`check_downgrade`]).
pub fn downgrade_result(
    target: FormatVersion,
    violations: Vec<Violation>,
) -> Result<(), Box<DowngradeError>> {
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Box::new(DowngradeError {
            target,
            violations,
        }))
    }
}

fn walk_object(obj: &MetadataObject, target: FormatVersion, out: &mut Vec<Violation>) {
    collect(
        obj.kind.0.as_str(),
        obj.name.as_str(),
        &obj.properties,
        target,
        out,
    );
    for form in &obj.form_bodies {
        walk_form(obj, form, target, out);
    }
    for child in &obj.children {
        walk_object(child, target, out);
    }
}

/// Нарушения ТЕЛА ФОРМЫ: свойства контролов, введённые позже таргета.
///
/// Отдельный проход от [`collect`], потому что источник гейта другой: у метаданных `since`
/// приходит из EDT-метамодели и лежит в [`FieldSpec::since`](crate::spec::common::FieldSpec),
/// а у форм метамодель версий не несёт ВООБЩЕ — гейт витнессирован ПЛАТФОРМОЙ и живёт в
/// `models/version_facts.jsonl` (ключ — пара «контрол + каноническое имя», см.
/// [`since_for_form_field`]). Без этого прохода даунгрейд 2.21→2.20 молча терял бы, например,
/// `AutoEditMode` у каждого поля ввода — ровно тот класс тихой потери, ради которого guard и
/// существует (§1.0).
fn walk_form(
    obj: &MetadataObject,
    form: &crate::ir::NamedFormBody,
    target: FormatVersion,
    out: &mut Vec<Violation>,
) {
    let where_ = format!("{}.{}", obj.name, form.name);
    // Форм-УРОВНЕВЫЕ свойства: владелец в витнесс-таблице — `Form` (корень `<Form>` дампа).
    collect_form_props(
        "Form",
        &where_,
        crate::spec::forms::form_root::form_root(),
        &form.body.attributes,
        target,
        out,
    );
    for item in &form.body.items {
        walk_form_item(&where_, item, target, out);
    }
}

fn walk_form_item(
    where_: &str,
    item: &crate::ir::FormItem,
    target: FormatVersion,
    out: &mut Vec<Violation>,
) {
    let kind = item.kind.as_str();
    if let Some(spec) = crate::spec::forms::controls::control_spec_for(kind) {
        let at = format!("{where_}.{}", item.name);
        collect_form_props(kind, &at, spec.properties, &item.properties, target, out);
        collect_form_props(kind, &at, spec.ext_info, &item.ext_info, target, out);
    }
    for child in &item.children {
        walk_form_item(where_, child, target, out);
    }
    for add in &item.additions {
        walk_form_item(where_, add, target, out);
    }
}

/// Свойства ОДНОГО форм-узла против витнессенной таблицы форм.
///
/// Нарушение — то же, что у метаданных: поле ПРИСУТСТВУЕТ, его `since` новее таргета И оно НЕ
/// равно умолчанию ЦЕЛЕВОЙ версии (умолчание не было бы потеряно — его в источнике и не было).
fn collect_form_props(
    owner: &str,
    at: &str,
    spec: &crate::spec::common::EntitySpec,
    props: &[(crate::ir::FieldId, crate::ir::value::PropertyValue)],
    target: FormatVersion,
    out: &mut Vec<Violation>,
) {
    let scope = crate::version::PropertyScope::Form { owner };
    for (id, value) in props {
        let Some(fs) = spec.fields().iter().find(|f| f.id == *id) else {
            continue;
        };
        let Some(since) = crate::version::since_for_form_field(owner, fs.name) else {
            continue;
        };
        if since.available_in(target) || is_target_default(scope, fs, value, target) {
            continue;
        }
        out.push(Violation {
            kind: owner.to_string(),
            object: at.to_string(),
            field: fs.name,
            since: since.0,
        });
    }
}

/// Равно ли значение УМОЛЧАНИЮ ЦЕЛЕВОЙ версии — критерий «потеря или не потеря».
///
/// Умолчание берётся ЕДИНЫМ швом [`crate::resolve::field_default`] и обязательно ДЛЯ
/// ЦЕЛЕВОЙ версии: вопрос гейта — «пропадёт ли что-нибудь, если записать таргет», а пропасть
/// может только то, что от таргета ОТЛИЧАЕТСЯ. Спрашивать умолчание версии ИСТОЧНИКА (или
/// версионно-нейтральную константу спека, как было) значило бы отвечать на другой вопрос:
/// умолчание — функция версии, и совпадение с чужим ничего не доказывает.
///
/// Умолчания нет ни у кого ([`crate::resolve::Resolution::Undetermined`]) ⇒ значение
/// умолчанием НЕ является, и присутствующее поле — потеря.
fn is_target_default(
    scope: crate::version::PropertyScope<'_>,
    fs: &crate::spec::common::FieldSpec,
    value: &crate::ir::value::PropertyValue,
    target: FormatVersion,
) -> bool {
    crate::resolve::field_default(scope, fs, Some(target)) == Some(value)
}

/// Свойства ОДНОЙ сущности: нарушение = поле ПРИСУТСТВУЕТ, его `since` новее таргета И
/// значение НЕ равно умолчанию ЦЕЛЕВОЙ версии (умолчание не было бы потеряно — его в
/// источнике и не было; см. [`is_target_default`]).
///
/// Граница берётся СВЕДЁННЫМ ответом ([`crate::version::availability`]): объявленная в
/// спеке ([`FieldSpec::since`], т.е. метамодель либо явный `gated`) — главнее, а где её нет,
/// спрашивается витнесс ПЛАТФОРМЫ по ключу реестра «владелец + элемент». Метамодель
/// проставляет `since` не везде, и без второго источника такое поле молча уезжало бы в
/// таргет, который его не знает.
fn collect(
    kind: &str,
    object: &str,
    props: &[(crate::ir::FieldId, crate::ir::value::PropertyValue)],
    target: FormatVersion,
    out: &mut Vec<Violation>,
) {
    let Some(spec) = spec_for(kind) else {
        // Вид без зарегистрированного спека нечем проверять — гейт молчит, а САМА запись
        // такого вида и так упадёт выше (§1.0: нет спека ⇒ вид не поддержан).
        return;
    };
    let scope = crate::resolve::scope_for_entity(spec.entity);
    for (id, value) in props {
        let Some(fs) = spec.fields().iter().find(|f| f.id == *id) else {
            continue;
        };
        let Some(since) = fs
            .since
            .or_else(|| crate::version::property_availability(scope, fs.name).since)
        else {
            continue;
        };
        if since.available_in(target) || is_target_default(scope, fs, value, target) {
            continue;
        }
        out.push(Violation {
            kind: kind.to_string(),
            object: object.to_string(),
            field: fs.name,
            since: since.0,
        });
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::ir::value::{PropertyValue, Token};
    use crate::ir::{FormBody, FormControlKind, FormItem, NamedFormBody, ObjectKind, Uuid};
    use crate::spec::forms::controls::radio_button::F_EXT_ORIENTATION;
    use crate::spec::forms::form_root::F_SHOW_COMMAND_BAR;

    const V220: FormatVersion = FormatVersion::new(2, 20);
    const V221: FormatVersion = FormatVersion::new(2, 21);

    /// Конфигурация 2.21 с формой, чьи свойства заданы `orientation`/`showCommandBar`.
    fn config_2_21(orientation: &str) -> Configuration {
        let mut cfg = Configuration::new();
        cfg.source_version = Some(V221);
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Товары", Uuid([0u8; 16]));
        let mut item = FormItem::new(FormControlKind::new("RadioButtonField"), "Переключатель", 1);
        item.ext_info.push((
            F_EXT_ORIENTATION,
            PropertyValue::Enum(Token::new(orientation)),
        ));
        let body = FormBody {
            attributes: vec![(F_SHOW_COMMAND_BAR, PropertyValue::Bool(false))],
            items: vec![item],
            ..FormBody::default()
        };
        obj.form_bodies.push(NamedFormBody {
            name: "ФормаЭлемента".into(),
            body,
            module: None,
            help: Vec::new(),
            help_resources: Vec::new(),
        });
        cfg.objects.push(obj);
        cfg
    }

    /// ПРАВИЛО 3: даунгрейд с РЕАЛЬНОЙ потерей отказывает и перечисляет ВСЕ потерянные
    /// свойства, а не первое. Оба свойства введены 2.21 и оба заданы значениями, которых
    /// в 2.20 не выразить.
    #[test]
    fn downgrade_with_real_loss_lists_every_lost_property() {
        let cfg = config_2_21("HorizontalIfPossible");
        let err = check_downgrade(&cfg, V220).expect_err("2.21 → 2.20 обязан отказать");
        let fields: Vec<&str> = err.violations.iter().map(|v| v.field).collect();
        assert!(
            fields.contains(&"orientation") && fields.contains(&"showCommandBar"),
            "перечислены не ВСЕ потери: {fields:?}"
        );
        assert_eq!(err.violations.len(), 2, "лишних нарушений быть не должно");
        let text = err.to_string();
        // Сообщение — часть контракта гейта, поэтому его печатаем (`-- --nocapture`).
        println!("{text}");
        assert!(text.contains("orientation"), "{text}");
        assert!(text.contains("showCommandBar"), "{text}");
        // И сообщение называет версию, которой хватило бы.
        assert!(text.contains("2.21"), "{text}");
    }

    /// …а значение, равное умолчанию ЦЕЛЕВОЙ версии, потерей НЕ является: его в источнике
    /// и не было. Витнесс: `orientation = Vertical` — ровно то, что Designer опускает.
    #[test]
    fn value_equal_to_the_target_default_is_not_a_loss() {
        let cfg = config_2_21("Vertical");
        let err = check_downgrade(&cfg, V220).expect_err("showCommandBar всё ещё теряется");
        let fields: Vec<&str> = err.violations.iter().map(|v| v.field).collect();
        assert_eq!(
            fields,
            vec!["showCommandBar"],
            "orientation=Vertical равен умолчанию 2.20 и потерей не является"
        );
    }

    /// Критерий «равно умолчанию» берётся ШВОМ `resolve::field_default` для ЦЕЛЕВОЙ версии.
    #[test]
    fn the_default_compared_against_is_the_targets_own() {
        let spec = crate::spec::forms::controls::control_spec_for("RadioButtonField").unwrap();
        let fs = spec
            .ext_info
            .fields()
            .iter()
            .find(|f| f.name == "orientation")
            .unwrap();
        let scope = crate::version::PropertyScope::Form {
            owner: "RadioButtonField",
        };
        assert_eq!(
            crate::resolve::field_default(scope, fs, Some(V220)),
            Some(&PropertyValue::Enum(Token::new("Vertical")))
        );
    }
}
