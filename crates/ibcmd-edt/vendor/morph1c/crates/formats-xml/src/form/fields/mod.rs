//! Таблично-управляемая ПРОЕКЦИЯ полей контролов формы (LANE-F-2): каноническое поле ↔
//! (EDT-тег, Designer-тег, кодек, политика пер-форматных дефолтов). Канонику (id/порядок/
//! value_kind) держат спеки `core/spec/forms/controls` (§1.6); здесь — форматные данные.
//!
//! # Кодеки (dual-encoding, сверены по корпусу SSL CommonForms)
//! * [`Codec::DataPath`] — БЛОКЕР №1: EDT `<tag xsi:type="form:DataPath"><segments>Путь`
//!   (ровно один `<segments>`, 569/569) ⟺ Designer `<Tag>Путь</Tag>` текстом.
//! * [`Codec::Color`] — EDT `<tag xsi:type="core:ColorRef"><color>Style.X|Palette.Y` ⟺
//!   Designer `style:X`/`pal:Y` текстом (префикс-биекция; иные префиксы — §1.0-ошибка).
//! * [`Codec::PictureRef`] — EDT `<tag xsi:type="core:PictureRef"><picture>Ref` ⟺ Designer
//!   `<Tag><xr:Ref>Ref</xr:Ref><xr:LoadTransparent>…` (LoadTransparent — денормализация вида
//!   ссылки: `StdPicture.*`→true, иначе false; сверяется на read, реконструируется на write).
//! * [`Codec::Value`] — nullable-скаляр xsi-вида через ОБЩИЙ [`crate::value_codec`] (тот же
//!   путь, что дескрипторные `<minValue>`/`<maxValue>`): EDT `<tag xsi:type="core:NumberValue">
//!   <value>N` / `core:StringValue` / `core:UndefinedValue` ⟺ Designer `<Tag xsi:type="xs:decimal">N`
//!   / `xs:string` / `xsi:nil`. Полный домен `ValueSpec` (Number-ДЕСЯТИЧНЫЙ, String, Undefined, …);
//!   невитнессированный xsi-тип → §1.0-отказ. Пример — InputField `minValue`/`maxValue` (ERP несёт
//!   и `99.99`, и `core:StringValue`, чего узкий Int-кодек не покрывал).
//! * [`Codec::CommonBool`] — EDT `<tag><common>true</common></tag>`(=true) / `<tag/>`(=false)
//!   ⟺ Designer absent(=true) / `<Tag><xr:Common>false</xr:Common></Tag>`.
//! * [`Codec::EnumMap`] — разные литералы форматов (EDT `ByContent` ⟺ Designer
//!   `UseContentHeight`); канон = EDT-литерал.
//!
//! # Политики (реконсиляция пер-форматных дефолтов, §1.6)
//! * [`Policy::Symmetric`] — оба формата эмитят значение как есть; отсутствует ⇒ нет в bag.
//! * [`Policy::OppositeBool`] — EDT эмитит только `true` (absent=false), Designer — только
//!   `false` (absent=true); канон-bag хранит только `true` (sparse против false).
//! * [`Policy::Keep`] — значение ВСЕГДА в bag: каждый ридер заполняет дефолт СВОЕГО формата
//!   при отсутствии, каждый писатель опускает по СВОЕМУ правилу. Дроп невосстановим,
//!   поэтому bag несёт значение постоянно (прецедент `buttonImportance`). ИСКЛЮЧЕНИЕ —
//!   свойство, которого ВЕРСИЯ ИСТОЧНИКА не знает: там омиссия не кодирует дефолт (тега в
//!   этой версии нет вовсе), заполнять нечем, и поле в bag НЕ кладётся (см.
//!   `policy::fill_allowed`; умолчание остаётся разрешимым из IR через `core::resolve`).
//! * [`Policy::PlatformDefault`] — как `Keep`, НО дефолт ПЛАТФОРМЫ (= литерал, который опускает
//!   Designer-дамп) в каноне НЕ хранится: «свойство не задано» = ОТСУТСТВИЕ поля в bag —
//!   одинаково у обоих ридеров (X-канон). Нужен там, где `edt_fill != des_fill`, иначе ридеры
//!   расходились бы на контроле, у которого свойство вообще не задано (витнесс:
//!   `GanttChartField` — EDT пишет `Enter`/`Left`, Designer молчит).
//! * [`Policy::EditMode`] — 4-литеральный канон {Auto,Enter,EnterOnInput,Directly} поверх
//!   `PlatformDefault` (`Enter`): EDT опускает `Directly`; Designer кодирует `Auto` ПАРОЙ
//!   `<EditMode>EnterOnInput` + `<AutoEditMode>true`, опускает `Enter` (сверено 280+6+1+1).

use super::{DESIGNER_FORM_NS, FormDialect, FormError};
use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::value_codec::{self, ValueDialect};
use crate::{TypeDialect, type_codec};
use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::ir::{FieldId, Lang};
use morph1c_core::spec::forms::controls::button as bt;
use morph1c_core::spec::forms::controls::form_field as ff;

/// Регион поля на EDT-стороне (Designer держит всё inline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Region {
    /// Тело элемента контрола.
    Body,
    /// `<extInfo xsi:type="form:…ExtInfo">`.
    Ext,
}

/// Кодек значения поля (см. модульный док).
#[derive(Debug, Clone, Copy)]
pub(super) enum Codec {
    Bool,
    Int,
    EnumTok,
    /// Свободный текст, канонически [`PropertyValue::Str`].
    Text,
    /// Свободный текст-ссылка, канонически [`PropertyValue::Ref`] (`commandSource`).
    RefText,
    /// Пер-форматный enum-мэппинг `(канон=EDT, Designer)`.
    EnumMap(&'static [(&'static str, &'static str)]),
    Localized,
    Color,
    PictureRef,
    /// Nullable-скаляр xsi-вида ([`PropertyValue::Value`] / [`crate::ir::value::ValueSpec`]) через
    /// ОБЩИЙ [`crate::value_codec`]. Полный домен (Undefined/String/Number-decimal/Bool/Date/
    /// Reference/AccountType) — оба диалекта декодируют в ОДИН канон (X by construction), write
    /// восстанавливает байты. Пример: InputField `minValue`/`maxValue`. §1.0: невитнессированный
    /// xsi-тип → ошибка кодека (никогда guess/passthrough).
    Value,
    DataPath,
    CommonBool,
    /// `Undefined`-значение-присутствие (Таблица `rowFilter`): EDT `<tag xsi:type=
    /// "core:UndefinedValue"/>` ⟺ Designer `<Tag xsi:nil="true"/>` — самозакрытые. Канон —
    /// `Bool(true)` (присутствие); отсутствие ⇒ нет в bag (Symmetric).
    UndefinedValue,
    /// Композит рамки `BorderDef` (PictureField): EDT `<border xsi:type="core:BorderDef">
    /// [<style>Single</style>]<width>1</width></border>` (эмитит ВСЕГДА; `<style>` есть ⟺
    /// канон `Single`, нет ⟺ `WithoutBorder`) ⟺ Designer `<Border width="1"><v8ui:style
    /// xsi:type="v8ui:ControlBorderType">WithoutBorder</v8ui:style></Border>` (эмитит ТОЛЬКО
    /// не-Single, через KEEP). Канон — `Enum(Single|WithoutBorder)`; ширина всегда 1 (иначе
    /// §1.0-ошибка). Политика — [`Policy::Keep`] (EDT всегда / Designer опускает `Single`).
    Border,
    /// Список значений выбора (repeatable, structured ValueList). Обрабатывается ОТДЕЛЬНО
    /// от табличного движка (см. [`read_choice_list_edt`]/[`read_choice_list_designer`]/
    /// [`emit_choice_list_edt`]/[`emit_choice_list_designer`]): канон — `List` из пар
    /// `[presentation:Localized, value:Value]`. НИКОГДА не доходит до `decode_*`/`render_*`.
    ChoiceList,
    /// Параметры выбора поля-ввода `choiceParameters` (repeatable, `name`+значение дизайн-тайм,
    /// обёрнутое в `FormChoiceListDesTimeValue`). Обрабатывается ОТДЕЛЬНО от табличного движка (см.
    /// [`read_choice_parameters_edt`]/[`read_choice_parameters_designer`]/[`emit_choice_parameters_edt`]/
    /// [`emit_choice_parameters_designer`]): канон — `List` из пар `[name:Str, value:Value]`. НИКОГДА
    /// не доходит до `decode_*`/`render_*`.
    ChoiceParameters,
    /// Командный параметр-ссылка кнопки (`parameter`/`Parameter`): EDT `<parameter
    /// xsi:type="core:ReferenceValue"><value>Document.Встреча</value></parameter>` ⟺ Designer
    /// `<Parameter xsi:type="xr:MDObjectRef">Document.Встреча</Parameter>` (текст-лист).
    /// Канон — `Ref` (SSL ×6⟷×6, одна форма кодировки).
    MdObjectRef,
    /// Связь по типу `typeLink` поля-ввода (structured). EDT `<typeLink>[<linkItem>N</linkItem>
    /// (омит 0)]<datapath xsi:type="form:DataPath"><segments>Путь</segments></datapath></typeLink>`
    /// ⟺ Designer `<TypeLink><DataPath>Путь</DataPath><LinkItem>N</LinkItem></TypeLink>` (ОБА
    /// дет-листа всегда; порядок ОБРАТНЫЙ EDT). Канон — `List([Ref(путь), Int(N)])`. Обрабатывается
    /// ОТДЕЛЬНО от табличного движка (см. [`read_type_link_edt`]/[`read_type_link_designer`]/
    /// [`emit_type_link_edt`]/[`emit_type_link_designer`]). НИКОГДА не доходит до `decode_*`/`render_*`.
    TypeLink,
    /// Связи параметров выбора `choiceParameterLinks` (repeatable, structured). EDT повторяемый
    /// `<choiceParameterLinks><name>Отбор.X</name><datapath xsi:type="form:DataPath"><segments>Путь`
    /// ⟺ Designer контейнер `<ChoiceParameterLinks><Link><Name>Отбор.X</Name><DataPath
    /// xsi:type="xs:string">Путь</DataPath><ValueChange>Clear</ValueChange></Link>…`. Канон —
    /// `List` пунктов `List([name:Str, path:Ref])`; `ValueChange` witnessed ТОЛЬКО `Clear`
    /// (Designer-каркас: сверяется на read, реконструируется на write; иное — §1.0-ошибка).
    /// Обрабатывается ОТДЕЛЬНО от табличного движка. НИКОГДА не доходит до `decode_*`/`render_*`.
    ChoiceParameterLinks,
    /// Скроллбар (SpreadsheetDocumentField): ENUM↔BOOL кросс-кодировка. EDT — enum-текст
    /// {`ScrollAuto`, `ScrollAlways`, `ScrollNever`} (как [`Codec::EnumTok`]) ⟺ Designer —
    /// BOOL (`true`=`ScrollAlways`, `false`=`ScrollNever`; ОПУСК=`ScrollAuto`). Канон —
    /// `Enum`. Пер-форматная омиссия (EDT опускает `ScrollNever`, Designer — `ScrollAuto`)
    /// делается [`Policy::Keep`]; `ScrollAuto` НИКОГДА не доходит до `render_designer`.
    ScrollBar,
    /// Стандартный период `{startDate, endDate}` (DynamicListTableExtInfo `period`). EDT —
    /// `<period><startDate>X</startDate><endDate>Y</endDate></period>` (2 листа, без ns) ⟺
    /// Designer — `<Period><v8:variant xsi:type="v8:StandardPeriodVariant">Custom</v8:variant>
    /// <v8:startDate>X</v8:startDate><v8:endDate>Y</v8:endDate></Period>` (3 ребёнка, ns `v8`).
    /// Канон — `List([Str(start), Str(end)])`. `variant`=`Custom` (Designer-каркас) сверяется на
    /// read (иное — §1.0-ошибка) и реконструируется на write; в канон НЕ входит.
    Period,
    /// Описание типа 1С ([`PropertyValue::Type`]) — через ОБЩИЙ [`type_codec`] (тот же путь, что
    /// `<valueType>`/`<Type>` реквизита): EDT `<tag><types>String</types><stringQualifiers/></tag>`
    /// ⟺ Designer `<Tag><v8:Type>xs:string</v8:Type><v8:StringQualifiers>…</Tag>`. Канон-`TypeSpec`
    /// одинаков (X by construction). Пример — `availableTypes` InputField.
    Type,
}

/// Константы кодировки [`Codec::Period`] (Designer-каркас `<v8:variant>`).
const PERIOD_VARIANT_XSI: &str = "v8:StandardPeriodVariant";
const PERIOD_VARIANT_VALUE: &str = "Custom";

/// Правило омиссии Designer-писателя для [`Policy::Keep`].
#[derive(Debug, Clone, Copy)]
pub(super) enum DesOmit {
    /// Эмитить всегда (Designer-обязательное поле, напр. `<Type>` кнопки).
    Never,
    /// Не эмитить никогда (EDT-only данность; дивергенция ловится X, не маскируется).
    Always,
    /// Опустить при равенстве литералу (дефолт Designer).
    Eq(&'static str),
}

/// Данные [`Policy::Keep`]: fill-литералы ридеров + правила омиссии писателей.
#[derive(Debug, Clone, Copy)]
pub(super) struct Keep {
    /// Чем EDT-ридер заполняет отсутствующее поле (дефолт EDT).
    pub edt_fill: &'static str,
    /// При каком значении EDT-писатель опускает поле; `None` = эмитит всегда.
    pub edt_omit: Option<&'static str>,
    /// Чем Designer-ридер заполняет отсутствующее поле (дефолт Designer).
    pub des_fill: &'static str,
    /// Правило омиссии Designer-писателя.
    pub des_omit: DesOmit,
}

/// Политика пер-форматных дефолтов поля (см. модульный док).
#[derive(Debug, Clone, Copy)]
pub(super) enum Policy {
    Symmetric,
    OppositeBool,
    Keep(Keep),
    // `DesKeep` (EDT presence-точный + Designer-Keep) СНЯТ. Единственным носителем был Page
    // `group`, а обоснование («EDT опускает у 2/78 страниц БЕЗ разрешимого правила») оказалось
    // артефактом ОДНОСТОРОННЕГО замера: правило нашлось кросс-витнесс-цензом — EDT опускает свой
    // ecore-дефолт `Horizontal`, Designer в тех же клетках пишет его ЯВНО; таблица ДИЗЪЮНКТНА на
    // SSL 898/898, ERP 12 220/12 220, coverage 36/36 ⇒ поле переведено на `Keep`. Прежде чем
    // вводить политику «одна сторона presence-точна, другая fill'ится», СНИМИ ценз по ОБОИМ
    // дампам одной конфигурации: односторонний замер не отличает «правила нет» от «не смотрел».
    /// ПЛАТФОРМЕННЫЙ ДЕФОЛТ (X-канон, §1.6).
    ///
    /// # ЗАКОН: у свойства ДВА разных «отсутствия»
    /// У части полей опущенный тег значит РАЗНОЕ в двух диалектах:
    /// * **ecore-дефолт EDT** — что значит омиссия НА EDT-СТОРОНЕ (`editMode` → `Directly`,
    ///   `headerHorizontalAlign` → `Auto`); это дефолт МЕТАМОДЕЛИ EDT, платформа его не знает;
    /// * **дефолт ПЛАТФОРМЫ** — что значит омиссия НА DESIGNER-СТОРОНЕ (`editMode` → `Enter`,
    ///   `headerHorizontalAlign` → `Left`); Designer-дамп пишет сама платформа и опускает РОВНО
    ///   дефолт свойства.
    ///
    /// В IR обязано храниться «ОТСУТСТВУЕТ ⇒ дефолт ПЛАТФОРМЫ», потому что именно платформенный
    /// дефолт пишется в cf: незаданный `editMode` уходит витнессованными байтами `Enter` (клетки
    /// 1/1), незаданный `headerHorizontalAlign` — ординалом `Left` (0). ecore-дефолт EDT
    /// платформенным НЕ является и потому хранится в bag ЯВНО.
    ///
    /// Следствие: `Policy::Keep` для таких полей НЕВЕРЕН — он клал в bag ПЕР-ФОРМАТНЫЙ fill, и у
    /// контрола, где свойство не задано ВООБЩЕ (ни один диалект тега не несёт), ридеры давали
    /// РАЗНЫЙ IR (витнесс `GanttChartField`: designer-дамп не несёт ни `<EditMode>`, ни
    /// `<HeaderHorizontalAlign>` — 0/17 форм, а EDT-дамп несёт `<editMode>Enter` 17/17 и
    /// `<headerHorizontalAlign>Left` 17/17).
    ///
    /// # Кросс-витнесс (ERP designer_8.3.27 ⟂ ERP edt, 122 429 сопоставленных по имени полей)
    /// * `editMode`: EDT `Enter` ⟺ Designer ОТСУТСТВУЕТ 86 533; EDT `EnterOnInput` ⟺ Designer
    ///   `EnterOnInput` 35 655; EDT ОТСУТСТВУЕТ ⟺ Designer `Directly` 241.
    /// * `headerHorizontalAlign`: EDT `Left` ⟺ Designer ОТСУТСТВУЕТ 122 142; `Center`/`Right`
    ///   зеркальны 276; EDT ОТСУТСТВУЕТ ⟺ Designer `Auto` 11.
    ///
    /// SSL (8.5.1) даёт ту же таблицу (+ пара `Auto` ⟺ `EnterOnInput`+`AutoEditMode`). Сочетания
    /// «оба опускают» в корпусе НЕТ — омиссии диалектов ДИЗЪЮНКТНЫ, поэтому канон восстановим.
    ///
    /// # Правила
    /// * read EDT: absent ⇒ `edt_fill` (ecore-дефолт EDT, ЯВНО); значение == `des_fill` ⇒ НЕ в bag;
    /// * read Designer: absent ⇒ НЕ в bag; значение ⇒ как есть;
    /// * write EDT: absent ⇒ эмитить `des_fill`; значение == `edt_omit` ⇒ опустить; иначе эмит;
    /// * write Designer: absent ⇒ опустить; иначе по `des_omit`.
    ///
    /// Обе омиссии остаются per-format ⇒ R byte-exact сохраняется на ОБЕИХ сторонах, а
    /// «свойство не задано» получает ОДНО каноническое написание (отсутствие в bag).
    PlatformDefault(Keep),
    /// `editMode` — 4-литеральный канон {Auto, Enter, EnterOnInput, Directly} с ПЛАТФОРМЕННЫМ
    /// дефолтом [`Policy::PlatformDefault`] (`Enter`, см. кросс-витнесс там же) и Designer-
    /// кодировкой `Auto` ПАРОЙ `<EditMode>EnterOnInput` + `<AutoEditMode>true`.
    EditMode,
    /// ВАЖНОСТЬ КНОПКИ (`buttonImportance`) — ТОЧНОЕ ПРИСУТСТВИЕ + Designer-омиссия
    /// платформенного дефолта.
    ///
    /// # ЗАКОН: fill ЗАПРЕЩЁН, потому что омиссии диалектов НЕ ДИЗЪЮНКТНЫ
    /// У `editMode`/`headerHorizontalAlign` ([`Policy::PlatformDefault`]) сочетания «ОБА
    /// диалекта опускают» в корпусе НЕТ, поэтому пер-форматный fill восстановим. У
    /// `buttonImportance` оно ЕСТЬ и МАССОВО: под компатом 8.3.27 свойство не существует
    /// вовсе — ВСЕ 80 252 кнопки ERP опускают тег в ОБОИХ диалектах (сплошной поиск подстроки
    /// по корпусу: `ButtonImportance` 0 вхождений, `buttonImportance` 0). Полная таблица
    /// кросс-витнесс-ценза по ТРЁМ конфигурациям (84 496 кнопок) — в доке
    /// [`bt::BUTTON_IMPORTANCE_DESIGNER_DEFAULT`].
    ///
    /// Значит ЛЮБОЙ fill-литерал здесь — домысел (§1.0). Правила:
    /// * read EDT / read Designer: тег есть ⇒ значение как есть; тега нет ⇒ **НЕ в bag**
    ///   (ОДНО каноническое написание «важность не задана» у обоих ридеров);
    /// * write EDT: в bag ⇒ эмит как есть; нет ⇒ опустить (чистая симметрия ⇒ R байт-точен);
    /// * write Designer: значение == `Normal` (дефолт ПЛАТФОРМЫ, Designer не эмитит его НИ РАЗУ
    ///   ни в одном корпусе) ⇒ опустить; нет в bag ⇒ опустить; иначе эмит.
    ///
    /// # ОСТАТОК (нужен ВЕРСИОННЫЙ ВХОД, отдельный класс)
    /// Присутствие тега в EDT-дампе ВЕРСИОННО: EDT 8.5.1 пишет `Normal` ЯВНО у обычной кнопки и
    /// опускает тег у кнопки-умолчания (там важность ПРОИЗВОДНАЯ — `Main`), EDT 8.3.27 не пишет
    /// тег НИКОГДА. Один и тот же IR («не задана») обязан на write дать РАЗНЫЙ EDT-байт для SSL
    /// и для ERP ⇒ X-равенство и R-байт-точность для этого поля НЕСОВМЕСТИМЫ, пока версия
    /// конфигурации не станет входом форм-кодека. Здесь выбрана R-точность (не фабрикуем ничего);
    /// остаточное X-неравенство (SSL: `Normal` ⟂ отсутствие 3 554 + производный `Main` ⟂
    /// отсутствие 471; coverage: 60) сведено в `normalize_form_for_x` и БАЙТ-НЕЙТРАЛЬНО —
    /// в cf оба состояния дают одну ячейку `[57]`.
    ButtonImportance,
}

/// Проекция ОДНОГО канонического поля в оба формата.
#[derive(Debug, Clone, Copy)]
pub(super) struct FieldProj {
    /// Канонический id (ключ спека `core/spec/forms/controls`).
    pub id: FieldId,
    /// EDT-тег (lowerCamel).
    pub edt: &'static str,
    /// Designer-тег (UpperCamel; ВНИМАНИЕ: бывают расхождения имён — `slaveItemsWidth` ⟺
    /// `ChildItemsWidth`, `hyperlink` ⟺ `Hiperlink` — и Designer-АТРИБУТЫ, см. `des_attr`).
    pub des: &'static str,
    /// Регион на EDT-стороне.
    pub region: Region,
    pub codec: Codec,
    pub policy: Policy,
    /// Exact XML 2.20 platform projection; later profiles retain the original policy.
    pub xml220_policy: Option<Policy>,
    /// Designer кодирует поле АТРИБУТОМ элемента контрола (`DisplayImportance`), не тегом.
    pub des_attr: bool,
}

impl FieldProj {
    pub(super) fn policy_for(
        &self,
        version: Option<morph1c_core::version::FormatVersion>,
    ) -> Policy {
        if version == Some(morph1c_core::version::FormatVersion::new(2, 20)) {
            self.xml220_policy.unwrap_or(self.policy)
        } else {
            self.policy
        }
    }
}

/// Конструктор строки таблицы (компактность объявлений).
pub(super) const fn fp(
    id: FieldId,
    edt: &'static str,
    des: &'static str,
    region: Region,
    codec: Codec,
    policy: Policy,
) -> FieldProj {
    FieldProj {
        id,
        edt,
        des,
        region,
        codec,
        policy,
        des_attr: false,
        xml220_policy: None,
    }
}

/// Разобрать fill/omit-литерал политики по кодеку поля. Литерал — РАЗРАБОТЧИКОВА
/// КОНСТАНТА таблицы (не корпусные данные): битый int здесь = баг таблицы, а не входа,
/// поэтому падаем громко (`expect`), а не молча подставляем 0.
fn parse_lit(codec: Codec, lit: &str) -> PropertyValue {
    match codec {
        Codec::Bool | Codec::CommonBool => PropertyValue::Bool(lit == "true"),
        Codec::Int => PropertyValue::Int(
            lit.parse::<i64>()
                .unwrap_or_else(|e| panic!("keep-int literal {lit:?} is not a valid i64: {e}")),
        ),
        _ => PropertyValue::Enum(Token::new(lit)),
    }
}

fn bag_get(bag: &[(FieldId, PropertyValue)], id: FieldId) -> Option<&PropertyValue> {
    bag.iter().find(|(k, _)| *k == id).map(|(_, v)| v)
}

mod common;
mod designer_read;
mod designer_write;
mod edt_read;
mod edt_write;
mod policy;
#[cfg(any())]
mod tests;

pub(crate) use common::*;
pub(crate) use designer_read::*;
pub(crate) use designer_write::*;
pub(crate) use edt_read::*;
pub(crate) use edt_write::*;
pub(crate) use policy::*;
