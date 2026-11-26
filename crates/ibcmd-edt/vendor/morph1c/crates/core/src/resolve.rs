//! Разрешение значения свойства ИЗ IR: явно заданное — либо умолчание версии.
//!
//! # Зачем отдельный модуль
//! Мешок свойств ([`crate::ir::MetadataObject::properties`], [`crate::ir::FormItem`]) несёт
//! ТОЛЬКО то, что было в источнике. Это даёт компактность (IR целой ERP — порядка 10 ГБ,
//! миллионы записей; копия умолчания в каждой записи удвоила бы его без единого бита новой
//! информации) — но сам по себе мешок на вопрос «каково значение свойства X у сущности Y»
//! не отвечает: отсутствие значит «действует умолчание», а какое именно — знает СПЕК.
//!
//! Здесь и живёт ответ. Умолчание не КОПИРУЕТСЯ в IR, оно из IR РАЗРЕШАЕТСЯ: мешок + спек
//! сущности + [`crate::ir::Configuration::source_version`]. Ответ ТОТАЛЕН — [`Resolution`]
//! различает три исхода, среди которых есть и честное «умолчание неизвестно» (§1.0: не
//! выдумывать значение, которого никто не засвидетельствовал).
//!
//! # ШОВ источника умолчаний
//! Умолчание берётся РОВНО в одном месте — [`field_default`]. Источников у него ДВА, и
//! правило их слияния записано в доке этой функции; версия входа — обязательная часть
//! вопроса, потому что умолчание в принципе может быть функцией версии.
//!
//! # ПРАВИЛО СЛИЯНИЯ УМОЛЧАНИЙ (кратко; полностью — в доке [`field_default`])
//! 1. Умолчание, ВИТНЕССИРОВАННОЕ платформой для ЗАПРОШЕННОЙ версии
//!    (`models/version_registry.jsonl`), — главнее.
//! 2. Витнесса для этой версии нет — берётся [`FieldSpec::default`]. Умолчание ДРУГОЙ
//!    версии не подставляется НИКОГДА.
//! 3. Нет и его — честное [`Resolution::Undetermined`].
//! 4. Витнесс реестра, ПРОТИВОРЕЧАЩИЙ спеку, спек не отменяет: он слабее (см. доку
//!    [`field_default`]), и расхождение — НАХОДКА, видимая через [`field_default_witness`].

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::{Configuration, FieldId, FormItem, MetadataObject};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::version::availability::{self, PropertyScope};
use crate::version::FormatVersion;

/// Ответ на вопрос «каково значение свойства».
///
/// Три исхода, и все три — данные, а не отговорки: значение из источника, значение по
/// умолчанию, либо честное «умолчания нет» (поле объявлено без него и в источнике
/// отсутствует — значит платформенное умолчание НЕ засвидетельствовано, и подставлять
/// нечего, §1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolution<'a> {
    /// Свойство ЗАДАНО в источнике (присутствует в мешке).
    Explicit(&'a PropertyValue),
    /// Свойство не задано — действует умолчание.
    Default(&'a PropertyValue),
    /// Свойство не задано, а умолчания у него НЕТ (не засвидетельствовано).
    Undetermined,
}

impl<'a> Resolution<'a> {
    /// Эффективное значение (явное либо умолчание); `None` — [`Resolution::Undetermined`].
    pub fn value(self) -> Option<&'a PropertyValue> {
        match self {
            Resolution::Explicit(v) | Resolution::Default(v) => Some(v),
            Resolution::Undetermined => None,
        }
    }

    /// Было ли значение ЗАДАНО в источнике (а не восстановлено умолчанием).
    pub fn is_explicit(self) -> bool {
        matches!(self, Resolution::Explicit(_))
    }
}

/// Чем подтверждено выданное умолчание — диагностика правила слияния (см.
/// [`field_default`]). Нужна тестам и обзорам: расхождение источников обязано быть ВИДНО,
/// а не растворяться в ответе.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultWitness {
    /// Умолчание подтверждено ПЛАТФОРМОЙ для запрошенной версии (версионный реестр) и не
    /// противоречит канону.
    Registry,
    /// Умолчание взято из канонического спека ([`FieldSpec::default`]): для запрошенной
    /// версии витнесса нет (или версия не задана / не покрыта прогоном).
    Spec,
    /// Реестр называет для этой версии ДРУГОЕ умолчание, чем спек. Выдаётся спековое (его
    /// витнесс сильнее — см. [`field_default`]), а расхождение — НАХОДКА.
    RegistryContradictsSpec,
    /// Умолчания нет ни у одного источника.
    Absent,
}

/// ШОВ: единственное место, где берётся УМОЛЧАНИЕ поля.
///
/// `scope` — сущность-владелец в ключах версионного реестра (см. [`scope_for_entity`]);
/// `version` — версия формата, в контексте которой спрашивают умолчание (обычно
/// [`Configuration::source_version`] либо таргет записи).
///
/// # ПРАВИЛО СЛИЯНИЯ ИСТОЧНИКОВ
/// Источников два, и они РАЗНОЙ СИЛЫ.
///
/// * **Версионный реестр** (`models/version_registry.jsonl`) — единственный источник с
///   ВЕРСИОННОЙ ОСЬЮ: он говорит, какое значение платформа вывела САМА там, где источник о
///   свойстве молчал, — отдельно для каждой снятой версии. Это и есть «умолчание версии».
/// * **[`FieldSpec::default`]** — версионно-нейтральная константа канонического спека.
///   Версии не различает, зато витнессирована САМЫМ сильным свидетелем, какой у проекта
///   есть: байт-точным round-trip'ом боевого корпуса (ERP/SSL). Неверное значение здесь
///   означало бы, что писатель опускает не то, и корпус бы не сошёлся.
///
/// Отсюда порядок:
/// 1. Есть витнесс реестра ДЛЯ ЗАПРОШЕННОЙ ВЕРСИИ, представимый в объявленном
///    [`FieldSpec::value_kind`], и он НЕ противоречит спеку — ответ его
///    ([`DefaultWitness::Registry`]). Если спек умолчания не объявляет вовсе, реестр
///    ЗАКРЫВАЕТ пробел: `Undetermined` превращается в засвидетельствованное умолчание.
/// 2. Витнесса для этой версии нет (версия не задана, не покрыта прогоном, свойство с
///    реестром не сопоставлено или прогон умолчания не установил) — [`FieldSpec::default`]
///    ([`DefaultWitness::Spec`]). Умолчание ДРУГОЙ версии не подставляется НИКОГДА: реестр
///    отвечает ровно за те версии, которые сняты, а «похожая версия» — это догадка (§1.0).
/// 3. Витнесс реестра ПРОТИВОРЕЧИТ спеку — побеждает спек
///    ([`DefaultWitness::RegistryContradictsSpec`]). Причина не в осторожности, а в
///    измеренной силе свидетелей: витнесс реестра — «платформа эмитила это значение там,
///    где источник молчал», и он НАДЁЖЕН в плотной области дампа (дескрипторы метаданных
///    выгружаются целиком) и НЕНАДЁЖЕН в разрежённой (тело формы), где «источник молчал»
///    неотличимо от «платформа тоже молчит». Witnessed: реестр называет умолчанием
///    `RadioButtonField.orientation` литерал `HorizontalIfPossible` (модальное значение
///    четырёх наблюдений), тогда как Designer опускает именно `Vertical` — и именно
///    `Vertical` требуется, чтобы cf-ординал сошёлся побайтно.
/// 4. Нет ни того, ни другого — `None` (тотальный ответ даёт [`Resolution::Undetermined`]).
pub fn field_default<'a>(
    scope: PropertyScope<'_>,
    field: &'a FieldSpec,
    version: Option<FormatVersion>,
) -> Option<&'a PropertyValue> {
    field_default_witness(scope, field, version).1
}

/// [`field_default`] + ЧЕМ подтверждён ответ (см. [`DefaultWitness`]).
pub fn field_default_witness<'a>(
    scope: PropertyScope<'_>,
    field: &'a FieldSpec,
    version: Option<FormatVersion>,
) -> (DefaultWitness, Option<&'a PropertyValue>) {
    let witnessed: Option<&'static PropertyValue> = version
        .and_then(|v| availability::registry_row(scope, field.name, v))
        .and_then(|row| row.default)
        .and_then(|text| typed_defaults().get(&(text, field.value_kind)));
    match (witnessed, field.default.as_ref()) {
        // Реестр подтвердил канон — ответ тот же, но теперь он привязан к ВЕРСИИ.
        (Some(w), Some(spec)) if w == spec => (DefaultWitness::Registry, Some(spec)),
        // Расхождение: побеждает спек, находка остаётся видимой.
        (Some(_), Some(spec)) => (DefaultWitness::RegistryContradictsSpec, Some(spec)),
        // Спек умолчания не знает — реестр закрывает пробел.
        (Some(w), None) => (DefaultWitness::Registry, Some(w)),
        (None, Some(spec)) => (DefaultWitness::Spec, Some(spec)),
        (None, None) => (DefaultWitness::Absent, None),
    }
}

/// ШОВ ПРАВИЛА 2: значение, которое ЦЕЛЕВАЯ версия ставит свойству, ОТСУТСТВУЮЩЕМУ в
/// источнике (см. [`crate::version::upgrade`]).
///
/// # Почему это НЕ [`field_default`]
/// Вопросы разные, и витнессы у них разные:
/// * [`field_default`] — «какое значение писатель ЭТОЙ версии ОПУСКАЕТ» (омиссия дампа);
/// * здесь — «какое значение платформа КЛАДЁТ туда, где источник о свойстве молчал».
///
/// Витнесс второго — прямой: прогон реестра (`models/version_registry.jsonl`,
/// `default_witness = emitted_where_source_silent`) записывает ровно те значения, которые
/// платформа ВЫВЕЛА САМА при загрузке источника, где этого свойства не было. Поэтому здесь
/// реестр ГЛАВНЕЕ спека — в отличие от [`field_default`], где он слабее (там его показание
/// об омиссии в разрежённой области неотличимо от «платформа тоже молчит»).
///
/// Witnessed-расхождение, ради которого шов и разведён: `RadioButtonField.orientation` —
/// Designer 8.5.1 ОПУСКАЕТ `Vertical` (144 контрола SSL), но, поднимая дамп 2.17/2.20, та же
/// платформа ставит `HorizontalIfPossible`. Оба факта верны и оба нужны: первый — писателю,
/// второй — правилу 2.
///
/// Порядок: (1) витнесс реестра для ЦЕЛЕВОЙ версии, представимый в объявленном
/// [`FieldSpec::value_kind`]; (2) [`field_default`] (умолчание цели); (3) `None` — значения
/// не знает никто, и подставлять нечего (§1.0: вызывающий обязан упасть, а не угадать).
pub fn field_upgrade_value<'a>(
    scope: PropertyScope<'_>,
    field: &'a FieldSpec,
    target: FormatVersion,
) -> Option<&'a PropertyValue> {
    let witnessed: Option<&'static PropertyValue> = availability::registry_row(scope, field.name, target)
        .and_then(|row| row.default)
        .and_then(|text| typed_defaults().get(&(text, field.value_kind)));
    match witnessed {
        Some(w) => Some(w),
        None => field_default(scope, field, Some(target)),
    }
}

/// Сущность канонического спека (`EntitySpec::entity`) → ключ ВЕРСИОННОГО РЕЕСТРА.
///
/// Реестр ключуется парой «владелец + элемент-владелец», спеки — одним дотированным кодом
/// сущности. Перевод ЯВНЫЙ (а не «поискать похожее»), и он проверяем: тест
/// `version_registry_keys` показывает, сколько полей спеков нашли себя в реестре, сколько
/// нет и почему.
///
/// Правила (в порядке применения):
/// * `<Контрол>ExtInfo` — тип-специфичный регион контрола; это тот же контрол;
/// * `FormRoot` — корень формы, в реестре он `Form`/`Form`;
/// * зарегистрированный вид контрола (`InputField`) — тело формы: `Form`/`<контрол>`;
/// * дотированный код (`Catalog.TabularSection.Attribute`) — владелец = ПЕРВЫЙ сегмент,
///   элемент = ПОСЛЕДНИЙ (реестр ключует элемент именем тега, а тег реквизита один и тот
///   же и в корне вида, и в его табличной части);
/// * простой код (`Catalog`) — владелец и элемент совпадают.
pub fn scope_for_entity(entity: &str) -> PropertyScope<'_> {
    let base = entity.strip_suffix("ExtInfo").unwrap_or(entity);
    if base == "FormRoot" {
        return PropertyScope::Form { owner: "Form" };
    }
    if crate::spec::forms::controls::control_spec_for(base).is_some() {
        return PropertyScope::Form { owner: base };
    }
    match (base.split('.').next(), base.rsplit('.').next()) {
        (Some(owner), Some(element)) if !owner.is_empty() && !element.is_empty() => {
            PropertyScope::Metadata { owner, element }
        }
        _ => PropertyScope::Unmapped,
    }
}

/// Разрешить свойство `id` сущности со спеком `spec` по её мешку `props`.
///
/// `None` — у сущности ВООБЩЕ нет такого поля (вопрос не про неё); иначе — тотальный
/// [`Resolution`].
pub fn resolve_field<'a>(
    spec: &'a EntitySpec,
    props: &'a [(FieldId, PropertyValue)],
    id: FieldId,
    version: Option<FormatVersion>,
) -> Option<Resolution<'a>> {
    let fs = spec.field(id)?;
    Some(resolve_with_spec(scope_for_entity(spec.entity), fs, props, version))
}

/// [`resolve_field`] по КАНОНИЧЕСКОМУ ИМЕНИ поля (`"orientation"`, `"lineNumberLength"`).
pub fn resolve_field_named<'a>(
    spec: &'a EntitySpec,
    props: &'a [(FieldId, PropertyValue)],
    name: &str,
    version: Option<FormatVersion>,
) -> Option<Resolution<'a>> {
    let fs = spec.fields().iter().find(|f| f.name == name)?;
    Some(resolve_with_spec(scope_for_entity(spec.entity), fs, props, version))
}

/// Общее тело разрешения при УЖЕ найденной спеке поля.
fn resolve_with_spec<'a>(
    scope: PropertyScope<'_>,
    fs: &'a FieldSpec,
    props: &'a [(FieldId, PropertyValue)],
    version: Option<FormatVersion>,
) -> Resolution<'a> {
    if let Some((_, v)) = props.iter().find(|(k, _)| *k == fs.id) {
        return Resolution::Explicit(v);
    }
    match field_default(scope, fs, version) {
        Some(d) => Resolution::Default(d),
        None => Resolution::Undetermined,
    }
}

/// Текст умолчания из реестра → типизированное значение ОБЪЯВЛЕННОГО спеком вида.
///
/// Реестр хранит умолчание ТЕКСТОМ (как оно стоит в дампе) — вид значения задаёт спек
/// (§1.6: тип поля не выбирает формат). Виды, которые в один токен не укладываются
/// (локализованная строка, описание типа, список, blob), из реестра НЕ восстанавливаются:
/// подобрать им значение по тексту значило бы догадываться (§1.0) — там отвечает спек.
fn parse_default(text: &'static str, kind: ValueKind) -> Option<PropertyValue> {
    match kind {
        ValueKind::Bool => match text {
            "true" => Some(PropertyValue::Bool(true)),
            "false" => Some(PropertyValue::Bool(false)),
            _ => None,
        },
        ValueKind::Int => text.parse::<i64>().ok().map(PropertyValue::Int),
        ValueKind::Str => Some(PropertyValue::Str(text.to_string())),
        ValueKind::Ref => Some(PropertyValue::Ref(text.to_string())),
        // Пустой текст — не литерал перечисления, а «значения нет»: восстанавливать нечего.
        ValueKind::Enum if !text.is_empty() => Some(PropertyValue::Enum(Token::new(text))),
        _ => None,
    }
}

/// Кэш типизированных умолчаний реестра: `(текст, объявленный вид) → значение`.
///
/// Ключ — ТЕКСТ, а не строка реестра: одинаковый текст даёт одинаковое значение, поэтому
/// кэш заодно дедуплицирует (различных текстов умолчаний в реестре — сотни, а строк —
/// тысячи). Строится один раз: [`field_default`] зовут в том числе с пути записи.
fn typed_defaults() -> &'static HashMap<(&'static str, ValueKind), PropertyValue> {
    static CACHE: OnceLock<HashMap<(&'static str, ValueKind), PropertyValue>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut map = HashMap::new();
        for p in crate::version::registry::REGISTRY_PROPERTIES {
            let Some(text) = p.default else { continue };
            for kind in [
                ValueKind::Bool,
                ValueKind::Int,
                ValueKind::Str,
                ValueKind::Ref,
                ValueKind::Enum,
            ] {
                if let Some(v) = parse_default(text, kind) {
                    map.insert((text, kind), v);
                }
            }
        }
        map
    })
}

/// Разрешить свойство ОБЪЕКТА МЕТАДАННЫХ по каноническому имени поля.
///
/// Версия берётся из САМОГО IR ([`Configuration::source_version`]) — вызывающему её
/// протаскивать не нужно. `None` — вид не зарегистрирован либо у него нет такого поля.
pub fn resolve_object<'a>(
    cfg: &Configuration,
    obj: &'a MetadataObject,
    name: &str,
) -> Option<Resolution<'a>> {
    let spec = crate::spec::registry::spec_for(obj.kind.as_str())?;
    let fs = spec.fields().iter().find(|f| f.name == name)?;
    Some(resolve_with_spec(
        scope_for_entity(spec.entity),
        fs,
        &obj.properties,
        cfg.source_version,
    ))
}

/// Разрешить свойство КОРНЯ конфигурации (вид `Configuration`) по каноническому имени.
pub fn resolve_configuration<'a>(cfg: &'a Configuration, name: &str) -> Option<Resolution<'a>> {
    let spec = crate::spec::registry::spec_for("Configuration")?;
    let fs = spec.fields().iter().find(|f| f.name == name)?;
    Some(resolve_with_spec(
        scope_for_entity(spec.entity),
        fs,
        &cfg.properties,
        cfg.source_version,
    ))
}

/// Разрешить свойство КОНТРОЛА ФОРМЫ по каноническому имени.
///
/// Ищет имя сперва в общих свойствах контрола, затем в его `extInfo` — ровно так же, как
/// оба региона живут в [`FormItem`]. `version` передаётся явно, потому что тело формы
/// читается и вне контекста целой конфигурации (сайдкар — самостоятельный файл).
///
/// Ключ реестра берётся из ВИДА КОНТРОЛА самого узла, а не из кода спека: `properties`
/// нескольких контролов может обслуживать один общий спек (`Decoration` у
/// `LabelDecoration`/`PictureDecoration`), и его код не назвал бы владельца.
pub fn resolve_control<'a>(
    item: &'a FormItem,
    name: &str,
    version: Option<FormatVersion>,
) -> Option<Resolution<'a>> {
    let spec = crate::spec::forms::controls::control_spec_for(item.kind.as_str())?;
    let scope = PropertyScope::Form {
        owner: item.kind.as_str(),
    };
    if let Some(fs) = spec.properties.fields().iter().find(|f| f.name == name) {
        return Some(resolve_with_spec(scope, fs, &item.properties, version));
    }
    let fs = spec.ext_info.fields().iter().find(|f| f.name == name)?;
    Some(resolve_with_spec(scope, fs, &item.ext_info, version))
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::ir::{FormControlKind, ObjectKind, Uuid};

    /// Витнессированный случай (RadioButtonField.orientation): 2.17-дамп тега НЕ несёт,
    /// значит в мешке его нет — и ответ обязан быть УМОЛЧАНИЕМ платформы, а не «нет данных».
    #[test]
    fn control_property_absent_resolves_to_platform_default() {
        let item = FormItem::new(FormControlKind::new("RadioButtonField"), "Переключатель", 1);
        let r = resolve_control(&item, "orientation", Some(FormatVersion::new(2, 17)))
            .expect("RadioButtonField declares orientation");
        assert_eq!(
            r,
            Resolution::Default(&PropertyValue::Enum(Token::new("Vertical")))
        );
        assert!(!r.is_explicit(), "значение не задано в источнике");
    }

    /// Заданное в источнике значение возвращается КАК ЕСТЬ и помечается явным.
    #[test]
    fn control_property_present_resolves_to_source_value() {
        let mut item =
            FormItem::new(FormControlKind::new("RadioButtonField"), "Переключатель", 1);
        item.ext_info.push((
            crate::spec::forms::controls::radio_button::F_EXT_ORIENTATION,
            PropertyValue::Enum(Token::new("HorizontalIfPossible")),
        ));
        let r = resolve_control(&item, "orientation", Some(FormatVersion::new(2, 21))).unwrap();
        assert!(r.is_explicit());
        assert_eq!(
            r.value(),
            Some(&PropertyValue::Enum(Token::new("HorizontalIfPossible")))
        );
    }

    /// Поле БЕЗ умолчания и без значения — честное «не знаю» (§1.0), а не выдуманное значение.
    #[test]
    fn field_without_default_is_undetermined() {
        let item = FormItem::new(FormControlKind::new("RadioButtonField"), "Переключатель", 1);
        let r = resolve_control(&item, "choiceList", Some(FormatVersion::new(2, 21))).unwrap();
        assert_eq!(r, Resolution::Undetermined);
        assert_eq!(r.value(), None);
    }

    /// Свойство объекта метаданных: версия берётся ИЗ IR, вызывающий её не передаёт.
    #[test]
    fn object_property_uses_source_version_from_ir() {
        let mut cfg = Configuration::new();
        cfg.source_version = Some(FormatVersion::new(2, 20));
        let obj = MetadataObject::new(ObjectKind::new("Catalog"), "Товары", Uuid([0u8; 16]));
        // Поле с умолчанием: отсутствие в мешке = умолчание.
        let r = resolve_object(&cfg, &obj, "hierarchical").expect("Catalog declares hierarchical");
        assert!(matches!(r, Resolution::Default(_)));
        // Несуществующее поле — вопрос не про эту сущность.
        assert!(resolve_object(&cfg, &obj, "нетТакогоПоля").is_none());
    }

    /// П. 1 правила слияния: умолчание ПОДТВЕРЖДЕНО реестром ДЛЯ ЗАПРОШЕННОЙ версии.
    /// `Catalog.hierarchical`: реестр видел `false` во всех трёх снятых версиях — ответ тот
    /// же, что у спека, но теперь он привязан к версии, а не «один на все».
    #[test]
    fn default_is_witnessed_by_the_registry_for_the_asked_version() {
        let spec = crate::spec::registry::spec_for("Catalog").unwrap();
        let fs = spec
            .fields()
            .iter()
            .find(|f| f.name == "hierarchical")
            .unwrap();
        let scope = scope_for_entity(spec.entity);
        for v in [(2, 17), (2, 20), (2, 21)] {
            let (w, value) = field_default_witness(scope, fs, Some(FormatVersion::new(v.0, v.1)));
            assert_eq!(w, DefaultWitness::Registry, "версия {v:?}");
            assert_eq!(value, Some(&PropertyValue::Bool(false)));
        }
    }

    /// П. 2: версия НЕ покрыта прогоном (или не задана) — отвечает спек, а НЕ умолчание
    /// соседней версии. Подставить чужую версию было бы догадкой (§1.0).
    #[test]
    fn unprobed_version_falls_back_to_spec_never_to_another_version() {
        let spec = crate::spec::registry::spec_for("Catalog").unwrap();
        let fs = spec
            .fields()
            .iter()
            .find(|f| f.name == "hierarchical")
            .unwrap();
        let scope = scope_for_entity(spec.entity);
        // 2.18 прогоном не снята.
        assert!(!crate::version::registry::covers_version(FormatVersion::new(2, 18)));
        assert_eq!(
            field_default_witness(scope, fs, Some(FormatVersion::new(2, 18))).0,
            DefaultWitness::Spec
        );
        assert_eq!(field_default_witness(scope, fs, None).0, DefaultWitness::Spec);
    }

    /// П. 3 правила слияния — ВИТНЕССИРОВАННОЕ расхождение источников.
    ///
    /// Реестр называет умолчанием `RadioButtonField.orientation` литерал
    /// `HorizontalIfPossible` (модальное значение четырёх наблюдений в 2.21), тогда как
    /// Designer опускает `Vertical` — и именно `Vertical` нужен, чтобы cf-ординал сошёлся
    /// побайтно. Побеждает спек; расхождение остаётся ВИДНЫМ через [`DefaultWitness`].
    #[test]
    fn registry_default_contradicting_the_spec_does_not_win_but_stays_visible() {
        let item = FormItem::new(FormControlKind::new("RadioButtonField"), "Переключатель", 1);
        let spec = crate::spec::forms::controls::control_spec_for("RadioButtonField").unwrap();
        let fs = spec
            .ext_info
            .fields()
            .iter()
            .find(|f| f.name == "orientation")
            .unwrap();
        let scope = PropertyScope::Form {
            owner: "RadioButtonField",
        };
        let (witness, value) = field_default_witness(scope, fs, Some(FormatVersion::new(2, 21)));
        assert_eq!(witness, DefaultWitness::RegistryContradictsSpec);
        assert_eq!(value, Some(&PropertyValue::Enum(Token::new("Vertical"))));
        // И через публичный шов — тот же ответ (иначе cf-ординал уехал бы с 1 на 2).
        let r = resolve_control(&item, "orientation", Some(FormatVersion::new(2, 21))).unwrap();
        assert_eq!(
            r,
            Resolution::Default(&PropertyValue::Enum(Token::new("Vertical")))
        );
    }

    /// Перевод «код сущности спека → ключ реестра» — ЯВНЫЙ и покрыт тестом.
    #[test]
    fn entity_codes_map_onto_registry_keys() {
        assert_eq!(
            scope_for_entity("Catalog"),
            PropertyScope::Metadata {
                owner: "Catalog",
                element: "Catalog"
            }
        );
        assert_eq!(
            scope_for_entity("Catalog.TabularSection.Attribute"),
            PropertyScope::Metadata {
                owner: "Catalog",
                element: "Attribute"
            }
        );
        assert_eq!(
            scope_for_entity("InputField"),
            PropertyScope::Form { owner: "InputField" }
        );
        assert_eq!(
            scope_for_entity("ButtonExtInfo"),
            PropertyScope::Form { owner: "Button" }
        );
        assert_eq!(
            scope_for_entity("FormRoot"),
            PropertyScope::Form { owner: "Form" }
        );
    }
}
