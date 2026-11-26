//! Общие примитивы канонического спека (ARCHITECTURE.md §1.4, §5 `spec/common`).
//!
//! Здесь живут типы, на которых строится КАЖДЫЙ спек сущности: [`FieldSpec`] (одно
//! поле) и [`EntitySpec`] (вид объекта/контрол как упорядоченный набор полей), плюс
//! разделяемые value-primitives ([`TriState`], [`Align`]) и хелперы по `TypeSpec`.
//!
//! Это СЕРДЦЕ §1.6: канонический `id`, `value_kind`, `default`, `since` и ПОРЯДОК
//! эмиссии каждого свойства заданы ЗДЕСЬ — ОДИН раз, для всех форматов. Формат не
//! волен выбрать иной тип/порядок: он лишь проецирует поле (см. [`crate::engine`]).

use serde::{Deserialize, Serialize};

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::version::{since_for_field_name, Since};

/// Внутри-видовая канонизация значения поля, применяемая движком на READ ДО
/// сравнения с `default` и записи в bag (ЛОКНУТОЕ требование ревью F2a, §1.6).
///
/// Раньше [`LocalizedSortByLang`] АЛЬФА-СОРТИЛ пары языков ПРЯМО в R-кодеке — это
/// латентно ломалось на двуязычном (ru-first) корпусе ERP: источник `ru`,`en` →
/// после сортировки `en`,`ru` → эмиттер писал `en`-first → НЕ byte-exact. На
/// одноязычном SSL (1 пара) сортировка была no-op, потому дефект не всплывал.
///
/// ПОЭТОМУ порядок языков теперь ПРЕД ХРАНЯЕТСЯ в источнике (§1.6 «cf-Source-order»,
/// как `TypeSpec.parts`): R остаётся байт-точным ДЛЯ КАЖДОГО формата, а канонизация
/// порядка языков для X-равенства (`read_edt(o) == read_designer(o)`) живёт ТОЛЬКО
/// в X-нормализации тесткита (`morph1c_testkit::normalize_for_x`), не в этом кодеке.
///
/// [`LocalizedSortByLang`] сохранён как МАРКЕР локализованного поля (его читают ~200
/// спек-файлов и потенциально будущая логика), но его `apply` — ТОЖДЕСТВО: он больше
/// НЕ переупорядочивает пары. Это делает `write∘read == id` byte-exact для двуязычных
/// объектов, не трогая X (где сортировка теперь и происходит).
///
/// Намеренно `enum` (данные), а НЕ fn-pointer: [`EntitySpec`] остаётся статической
/// `const`-таблицей, ревьюимой как данные (без исполняемого кода в спеке).
///
/// [`LocalizedSortByLang`]: Normalize::LocalizedSortByLang
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Normalize {
    /// Без изменений — значение уже канонично в любой проекции.
    Identity,
    /// [`PropertyValue::Localized`]: МАРКЕР локализованного поля. Порядок языков
    /// ПРЕД ХРАНЯЕТСЯ в источнике (byte-exact R); канонизация порядка для X — в
    /// X-нормализации тесткита, НЕ здесь (§1.6). `apply` — тождество (см. тип).
    LocalizedSortByLang,
}

impl Normalize {
    /// Применить нормализацию к декодированному значению (идемпотентно).
    ///
    /// Движок зовёт это на READ для КАЖДОГО поля перед дефолт-сравнением. Вид
    /// значения не меняется (нормализация канонизирует НАГРУЗКУ, не тип), так что
    /// последующая проверка `value_kind` (§1.6) остаётся в силе.
    ///
    /// [`LocalizedSortByLang`](Normalize::LocalizedSortByLang) — ТОЖДЕСТВО (порядок
    /// языков сохраняется из источника, byte-exact R); лэнг-сортировка для X-равенства
    /// перенесена в [`morph1c_testkit::normalize_for_x`], чтобы не ломать R.
    pub fn apply(self, value: PropertyValue) -> PropertyValue {
        match self {
            // Оба варианта — тождество на R-пути: порядок языков локализованной строки
            // берётся ИЗ ИСТОЧНИКА (byte-exact для каждого формата). Канон-порядок для
            // X-сравнения задаётся в X-нормализации тесткита, не в этом кодеке (§1.6).
            Normalize::Identity | Normalize::LocalizedSortByLang => value,
        }
    }
}

/// Каноническая спецификация ОДНОГО поля (свойства) сущности.
///
/// Несёт минимум по ТЗ F2a: канонический `id`, `value_kind`, `default` (для
/// компактности/омиссии, §1.1), `since` (версионный гейт, §1.5) — и участвует в
/// детерминированном порядке эмиссии (порядок = индекс поля в [`EntitySpec`]).
///
/// Именно из этих полей движок выводит read/write ОБОИХ форматов одинаково:
/// значение хранится под `id`, проверяется на `value_kind`, опускается если равно
/// `default`, гейтится `since`. Формат лишь говорит, ГДЕ оно лежит (проекция).
#[derive(Debug, Clone)]
pub struct FieldSpec {
    /// Канонический формат-нейтральный id (ключ §1.6). Уникален в пределах спека.
    pub id: FieldId,
    /// Человекочитаемое каноническое имя поля (для диагностики/диффа).
    pub name: &'static str,
    /// Объявленный вид значения. Движок ОБЯЗАН проверить соответствие
    /// декодированного значения этому виду — так формат лишён свободы выбрать тип.
    pub value_kind: ValueKind,
    /// УМОЛЧАНИЕ поля: при записи значение, равное ему, ОПУСКАЕТСЯ; в мешке IR оно НЕ
    /// хранится копией — «нет в мешке» и значит «действует умолчание».
    ///
    /// Это ИСТОЧНИК умолчания, но спрашивать его напрямую не следует: единственный шов —
    /// [`crate::resolve::field_default`] (он же даёт тотальный ответ
    /// [`crate::resolve::Resolution`] и переедет на версионный реестр умолчаний, не трогая
    /// потребителей).
    ///
    /// `None` = умолчание НЕ засвидетельствовано. Тогда отсутствие значения — честное «не
    /// знаю» (§1.0), а не повод что-то подставить: движок чтения на таком поле поднимает
    /// [`crate::engine::EngineError::MissingRequired`], а ридеры форм его не материализуют.
    pub default: Option<PropertyValue>,
    /// Внутри-видовая канонизация значения, применяемая движком на READ ДО
    /// дефолт-сравнения (§1.6). Делает X-равенство СТРУКТУРНЫМ. Для большинства
    /// полей — [`Normalize::Identity`].
    pub normalize: Normalize,
    /// Версия формата, в которой поле появилось (§1.5). `None` = было всегда в
    /// поддерживаемом диапазоне. В таргет старее `since` поле не эмитится.
    pub since: Option<Since>,
    /// X-ИСКЛЮЧЕНИЕ (§1.6): поле — ФОРМАТ-ЛОКАЛЬНАЯ денормализованная данность, которой
    /// у другого формата НЕТ аналога в дескрипторе (напр. EDT-only `<help>`-блок корня
    /// InformationRegister: 20 объектов несут его в `.mdo`, Designer — нигде). Такое
    /// поле УЧАСТВУЕТ в R (нужно для byte-exact round-trip своего формата), но
    /// ИСКЛЮЧЕНО из X-кросс-сравнения (`morph1c_testkit::cross_format_equal`), иначе
    /// EDT(`Bool(true)`)≠Designer(default) дало бы ложное расхождение. Это НЕ ослабляет
    /// X для обычных полей — только точечно для денормализованных. Дефолт `false`.
    pub x_ignore: bool,
}

impl FieldSpec {
    /// Поле без дефолта (присутствует всегда, когда доступно по версии).
    ///
    /// `since` НЕ передаётся руками: он ПРОСТАВЛЯЕТСЯ ЦЕНТРАЛЬНО по каноническому имени
    /// поля из since-таблицы, сгенерированной из инвентаря (§1.5,
    /// [`since_for_field_name`]). Так версия введения имеет РОВНО ОДИН источник правды —
    /// EDT-метамодель, — а не расползается копиями по 156 спек-файлам, где она
    /// неизбежно разошлась бы с инвентарём. Явный [`gated`](Self::gated) поверх остаётся
    /// (переопределение для полей, которых в метамодели нет).
    pub fn required(id: FieldId, name: &'static str, value_kind: ValueKind) -> Self {
        FieldSpec {
            id,
            name,
            value_kind,
            default: None,
            normalize: Normalize::Identity,
            since: since_for_field_name(name),
            x_ignore: false,
        }
    }

    /// Поле с дефолтом (опускается при записи, восстанавливается при чтении).
    /// `since` проставляется централизованно — см. [`required`](Self::required).
    pub fn with_default(
        id: FieldId,
        name: &'static str,
        value_kind: ValueKind,
        default: PropertyValue,
    ) -> Self {
        FieldSpec {
            id,
            name,
            value_kind,
            default: Some(default),
            normalize: Normalize::Identity,
            since: since_for_field_name(name),
            x_ignore: false,
        }
    }

    /// Версионный гейт `Since` поверх существующего спека поля (§1.5).
    pub fn gated(mut self, since: Since) -> Self {
        self.since = Some(since);
        self
    }

    /// Нормализация поверх существующего спека поля (§1.6).
    pub fn normalized(mut self, normalize: Normalize) -> Self {
        self.normalize = normalize;
        self
    }

    /// Пометить поле X-исключённым (формат-локальная денормализация, см. [`Self::x_ignore`]).
    pub fn x_ignored(mut self) -> Self {
        self.x_ignore = true;
        self
    }

    /// Канонизировать декодированное значение по [`Normalize`] этого поля. Движок
    /// зовёт это на READ ДО дефолт-сравнения, чтобы X-равенство было структурным.
    pub fn normalize(&self, value: PropertyValue) -> PropertyValue {
        self.normalize.apply(value)
    }

    /// Считается ли `value` равным дефолту (тогда оно опускается при записи).
    pub fn is_default(&self, value: &PropertyValue) -> bool {
        self.default.as_ref() == Some(value)
    }
}

/// Канонический ДЕФОЛТ поля `picture` (`PictureRef`-кодек): `List([Str(""), Bool(true)])` —
/// «картинки нет». Флаг `true` — не догадка: cf кодирует ПУСТУЮ картинку слотом
/// `loadTransparent=1` (сверено корпусом), и то же даёт правило реконструкции для пустой
/// ссылки.
pub fn picture_ref_default() -> PropertyValue {
    PropertyValue::List(vec![
        PropertyValue::Str(String::new()),
        PropertyValue::Bool(true),
    ])
}

/// Каноническое поле `picture` вида-с-командами / подсистемы (кодек `PictureRef`).
///
/// IR — `List([Str(ref), Bool(loadTransparent)])`. Флаг — РЕАЛЬНОЕ, задаваемое пользователем
/// свойство Designer/`.cf` (witnessed `CommonPicture.инт_Кафка` c `LoadTransparent=true`
/// ПРОТИВ выводимого правила), а НЕ константа/дериват — раньше он моделировался
/// «выводимым из префикса ссылки», и designer→designer МОЛЧА писал бы `false` (§1.0-потеря).
///
/// **X-исключение.** У EDT аналога НЕТ — доказано платформой: на designer→edt→cf→designer
/// платформа САМА теряет флаг и восстанавливает его тем же правилом (`.fixtures/versions/
/// README.md`). Значит правило — ДЕФОЛТ РЕКОНСТРУКЦИИ, а не инвариант: EDT-ридер выводит
/// флаг им же, а X-сравнение поля отключено (`x_ignore`), иначе объект-нарушитель дал бы
/// ложное расхождение форматов. Сам `ref` при этом остаётся под R (байт-точность каждого
/// формата) и под cf_compare-гейтом (designer→cf vs эталон).
pub fn picture_ref_field(id: FieldId) -> FieldSpec {
    FieldSpec::with_default(id, "picture", ValueKind::List, picture_ref_default()).x_ignored()
}

/// Один СЛОТ дочерней коллекции сущности (child-objects substrate, §1.1/§1.6,
///). Форматно-нейтрален: имя XML-тега даёт ПРОЕКЦИЯ
/// (см. `formats_xml::LocusMap::child_collection`), не спек.
///
/// Дочерняя сущность моделируется ТЕМ ЖЕ [`crate::ir::MetadataObject`] рекурсивно
/// (§1.1 «одна сущность — один тип»): движок рекурсирует в [`child_kind`]-спек так
/// же, как для корня. Порядок слотов в [`EntitySpec::children`] — канонический
/// порядок эмиссии групп детей (как `fields`); внутри слота порядок экземпляров —
/// authored, сохраняется позиционно в `MetadataObject.children` (§1.6).
///
/// [`child_kind`]: ChildSlot::child_kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildSlot {
    /// Канонический формат-нейтральный код коллекции (`"EnumValue"`, `"Dimension"`,
    /// …). НЕ имя XML-тега — проекция отображает его в edt/designer-тег.
    pub collection: &'static str,
    /// Полный dotted-kind дочернего вида (`"Enum.EnumValue"`) для lookup его
    /// [`EntitySpec`] через `crate::spec::registry::spec_for`.
    pub child_kind: &'static str,
}

/// Описание ОДНОЙ категории платформенно-сгенерированного типа вида (§1.6/§3.5).
///
/// Это METAMODEL-данные канонического спека: один вид (Catalog/Enum/InformationRegister/
/// …) несёт НЕСКОЛЬКО сгенерированных типов разных категорий (Manager/Ref/Object/List/…),
/// у которых опаковые (не выводимые) type-id GUID'ы захватываются каркасом `producedTypes`
/// (EDT) / `InternalInfo` (Designer). Порядок и имена этих категорий РАСХОДЯТСЯ между
/// форматами, поэтому IR хранит КАНОНИЧЕСКИЙ порядок (= порядок этого массива = EDT-
/// порядок), а каждая проекция переставляет к своему формату.
///
/// Данные ЖИВУТ В СПЕКЕ вида (per-kind `pub const PRODUCED_CATEGORIES` в
/// `core/spec/metadata/<вид>.rs`), а НЕ в закрытом match-арме формата (§1.6: канон — в
/// спеке, формат — лишь проекция). Обобщённый кодек `formats_xml::produced_types` читает
/// эту таблицу через реестр [`crate::spec::produced_categories_for`] и проецирует её в оба
/// XML-формата — добавление вида с `producedTypes` требует правки ТОЛЬКО его спек-файла
/// (parallel-safe fan-out, зеркало R1 std-attrs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducedCategory {
    /// Канонический код категории в IR (`"Container"`/`"Ref"`/`"List"`/`"Manager"`/…).
    /// Хранится в `GeneratedType::category` — формат-нейтральный ключ X-равенства.
    pub category: &'static str,
    /// Local-name EDT-тега в `<producedTypes>` (`"containerType"`/`"refType"`/…).
    pub edt_tag: &'static str,
    /// Значение Designer-атрибута `category` (`"DefinedType"`/`"Ref"`/`"List"`/…).
    /// Совпадает с IR `category` у Enum, но НЕ у DefinedType (IR=`Container`,
    /// Designer=`DefinedType`), поэтому хранится отдельно.
    pub designer_category: &'static str,
    /// Designer type-name prefix (`"DefinedType"`/`"EnumRef"`/…) — даёт
    /// `name="<prefix>.<ObjName>"`.
    pub designer_type_name: &'static str,
    /// Индекс этой категории в DESIGNER-нативном порядке эмиссии (для перестановки).
    pub designer_order: u8,
}

/// Каноническая спецификация СУЩНОСТИ (вид объекта / контрол / вспом. часть):
/// упорядоченный список полей. Порядок полей = ПОРЯДОК ЭМИССИИ (§3.2: «порядок
/// эмиссии спека = порядок эталона», сравнение детей позиционное).
///
/// Один [`EntitySpec`] на сущность (§1.4): из него движок выводит read/write для
/// КАЖДОГО формата. Это структурная гарантия §1.6 — нет места рукописному
/// пер-форматному ридеру, который мог бы разойтись.
#[derive(Debug, Clone)]
pub struct EntitySpec {
    /// Канонический код сущности (`Catalog`, `InputField`, …) — для диагностики.
    pub entity: &'static str,
    /// Поля в каноническом порядке эмиссии. Индекс = позиция эмиссии.
    pub fields: &'static [FieldSpec],
    /// Слоты дочерних коллекций в каноническом порядке эмиссии групп детей
    /// (child-objects substrate, §1.1/§1.6). Дефолт `&[]` — лист-вид без детей
    /// (большинство видов F2b); существующие виды не затронуты.
    pub children: &'static [ChildSlot],
}

impl EntitySpec {
    /// Спека поля по каноническому id, если есть в этой сущности.
    pub fn field(&self, id: FieldId) -> Option<&FieldSpec> {
        self.fields.iter().find(|f| f.id == id)
    }

    /// Поля в каноническом порядке эмиссии.
    pub fn fields(&self) -> &[FieldSpec] {
        self.fields
    }

    /// Слоты дочерних коллекций в каноническом порядке (child-objects substrate).
    pub fn children(&self) -> &[ChildSlot] {
        self.children
    }
}

/// Трёхзначное состояние (`Auto` / `Use` / `DontUse`) — частый вид свойства 1С,
/// семантически это [`PropertyValue::Enum`] с фиксированным набором литералов.
/// Хелпер: канонизирует литералы единообразно для всех форматов (§1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriState {
    /// `Auto`
    Auto,
    /// `Use`
    Use,
    /// `DontUse`
    DontUse,
}

impl TriState {
    /// Каноническое имя литерала (одинаково во всех форматах).
    pub fn token(self) -> &'static str {
        match self {
            TriState::Auto => "Auto",
            TriState::Use => "Use",
            TriState::DontUse => "DontUse",
        }
    }
}

// ============================================================================
// Декларация платформенного блока `standardAttributes` (child-objects substrate).
//
// Это ДАННЫЕ канонического спека (§1.4/§1.6): один вид объекта (Catalog, Document,
// Task, …) несёт фикс-набор предопределённых стандартных атрибутов, у каждого —
// одинаковая «форма» листьев, различающаяся ТОЛЬКО данными: набор+порядок атрибутов,
// name-определённые константы, variable-слоты, EDT-разрежённая последовательность
// листьев и Designer-DENSE 25-листовый регион, плюс имя корневого тега-обёртки.
//
// Тип формат-нейтрален (ни `Element`, ни имён XML-тегов формата в «структурном»
// смысле — только предопределённые платформой local-name'ы листьев, которые
// ИДЕНТИЧНЫ в EDT и Designer по родам). Обобщённый кодек `formats_xml` читает эту
// таблицу и проецирует её в оба XML-формата, устраняя пер-видовой рукописный кодек
// (§1.6: канон — здесь, формат — лишь проекция).
// ============================================================================

/// Какой из name-определённых констант атрибута адресует лист (индекс в
/// [`AttrDecl::name_consts`]). Платформа задаёт для некоторых листьев значение,
/// ВЫВОДИМОЕ из имени атрибута (напр. `fillChecking`: Date⇒ShowError иначе DontCheck;
/// `TypeReductionMode`: Owner⇒Deny иначе TransformValues). Здесь — лишь индекс; сами
/// значения лежат в [`AttrDecl::name_consts`] в том же порядке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameConstWhich {
    /// `name_consts[0]`.
    C0,
    /// `name_consts[1]`.
    C1,
}

impl NameConstWhich {
    /// Индекс в [`AttrDecl::name_consts`].
    pub const fn index(self) -> usize {
        match self {
            NameConstWhich::C0 => 0,
            NameConstWhich::C1 => 1,
        }
    }
}

/// Один предопределённый стандартный атрибут: его платформенное имя + значения
/// name-определённых констант в порядке [`NameConstWhich`]. Пустой `name_consts` —
/// у вида нет name-определённых констант (все — литеральные `Text`/`Trm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrDecl {
    /// Платформенное имя атрибута (`Ref`, `Date`, `LineNumber`, …) — одинаково у EDT
    /// (`<name>`) и Designer (`name="…"`).
    pub name: &'static str,
    /// Значения name-определённых констант этого атрибута (напр. `["ShowError"]` для
    /// `fillChecking` у Date). Индексируется [`NameConstWhich`].
    pub name_consts: &'static [&'static str],
}

/// Вид variable-слота IR-записи атрибута: определяет дефолт (для сборки
/// `default_record`) и способ кодирования в обоих форматах.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarSlotKind {
    /// Локализованная строка (`synonym`/`toolTip`/`format`/`editFormat`). Дефолт `[]`.
    Loc,
    /// Свободный текст (`comment`). Дефолт `""`.
    Str,
    /// Presence-bool (`fillFromFillingValue`). Дефолт `false`.
    Bool,
    /// Перечислимый литерал с дефолтом-литералом (Catalog variable `fillChecking`/
    /// `fullTextSearch`). Дефолт — `Enum(default)`.
    Enum(&'static str),
    /// Nullable-скаляр (`fillValue`). Дефолт — `Value(Undefined)`.
    Value,
    /// Список связей параметров выбора (`choiceParameterLinks`). Дефолт `[]`.
    Cpl,
    /// Список параметров выбора (`choiceParameters`, структурный List — codec
    /// ChoiceParameters). Дефолт `[]`. ERP-корпус вскрыл его на std-attrs.
    Cp,
}

/// Один лист EDT-блока `<standardAttributes>` в ПОЗИЦИОННОМ порядке. EDT разрежён:
/// variable-листья опускаются при дефолте; name-константы — при omit-значении.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdtLeaf {
    /// Литеральный текст-лист `<tag>text</tag>` (напр. `dataHistory`=Use).
    ConstText {
        /// Local-name листа.
        tag: &'static str,
        /// Литеральный текст.
        text: &'static str,
    },
    /// `<name>ИмяАтрибута</name>`.
    Name,
    /// `<name>ИмяАтрибута</name>`, ДОПОЛНИТЕЛЬНО хранимый в variable-слоте (`Str`).
    ///
    /// Нужен видам, у которых альтернативные наборы root-атрибутов НЕ различимы по
    /// длине (ERP-witnessed AccountingRegister: `[PeriodAdjustment,…]` и `[…,RecordType,…]`
    /// — оба 12): кодек выбирает вариант по ПОСЛЕДОВАТЕЛЬНОСТИ ИМЁН — на read имена
    /// наблюдаемы (EDT `<name>` / Designer `name="…"`), а на write берутся ИЗ ЭТОГО слота
    /// IR-записи (иначе выбор варианта на записи был бы неоднозначен — §1.0: не догадка).
    /// Слот обязан быть `VarSlotKind::Str`; прочитанное имя СВЕРЯЕТСЯ с выбранным
    /// вариантом (расхождение → ошибка).
    NameVar(usize),
    /// Опциональный localized-лист variable-слота (омитится, если пуст): `(tag, slot)`.
    OptLoc(&'static str, usize),
    /// Опциональный plain-str-лист variable-слота (омитится, если пуст): `(tag, slot)`.
    OptStr(&'static str, usize),
    /// Опциональный presence-bool variable-слота (омитится при false): `(tag, slot)`.
    OptBool(&'static str, usize),
    /// Опциональный enum variable-слота (омитится при значении == дефолт слота):
    /// `(tag, slot)`.
    OptEnum(&'static str, usize),
    /// Опциональный choiceParameterLinks variable-слота (омитится при пустом списке):
    /// `(tag, slot)`.
    OptCpl(&'static str, usize),
    /// Опциональный choiceParameters variable-слота: EDT — MULTI-SIBLING (`<tag>` на связку,
    /// подряд; НОЛЬ сиблингов при пустом списке): `(tag, slot)`. ERP-witnessed на std-attrs.
    OptCp(&'static str, usize),
    /// Опциональный linkByType variable-слота (омитится при пустом пути): `(tag, slot)`.
    /// IR-нагрузка Str (как у реквизитного linkByType; codec LinkByType).
    OptLbt(&'static str, usize),
    /// Обязательный Value-xsi-лист variable-слота (`fillValue` — всегда present):
    /// `(tag, slot)`.
    ReqValue(&'static str, usize),
    /// name-определённый константный текст-лист (`fillChecking`/`fullTextSearch`),
    /// эмитится ТОЛЬКО если значение != `omit` (иначе опускается).
    NameConst {
        /// Local-name листа.
        tag: &'static str,
        /// Какая из name-констант атрибута.
        which: NameConstWhich,
        /// Значение-омиссии (при нём лист не эмитится).
        omit: &'static str,
    },
    /// Константный `<tag xsi:...>` = Undefined-скаляр (`minValue`/`maxValue`).
    ConstValueUndef {
        /// Local-name листа.
        tag: &'static str,
    },
}

/// Один лист Designer-DENSE-региона `<xr:StandardAttribute>` (фикс 25 листьев,
/// всегда все). Порядок — как в эталоне.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenseLeaf {
    /// Пустой самозакрывающийся `<xr:Tag/>`.
    Empty,
    /// Самозакрывающийся `<xr:Tag xsi:nil="true"/>`.
    Nil,
    /// Литеральный текст `<xr:Tag>text</xr:Tag>`.
    Text(&'static str),
    /// localized variable-слот (v8:item).
    VarLoc(usize),
    /// plain-str variable-слот (Task/BP Comment).
    VarStr(usize),
    /// presence-bool variable-слот (Catalog FillFromFillingValue).
    VarBool(usize),
    /// enum variable-слот (Catalog FillChecking/FullTextSearch variable).
    VarEnum(usize),
    /// Value-xsi variable-слот (FillValue).
    VarValue(usize),
    /// choiceParameterLinks variable-слот (Catalog).
    VarCpl(usize),
    /// choiceParameters variable-слот (`<xr:ChoiceParameters>` c `<app:item>`; пусто —
    /// self-closing). ERP-witnessed на std-attrs (Catalog).
    VarCp(usize),
    /// linkByType variable-слот (`<xr:LinkByType>` c DataPath/LinkItem; пусто —
    /// self-closing). IR-нагрузка Str. ERP-witnessed на std-attrs (AccountingRegister).
    VarLbt(usize),
    /// name-определённый константный текст-лист (FillChecking/FullTextSearch/TRM).
    VarNameConst(NameConstWhich),
}

/// Один Designer-лист листа-имени-тега (Designer local-name) + его правило.
pub type DenseLeafRule = (&'static str, DenseLeaf);

/// Вариант блока: КОРЕНЬ вида (через `<root_elem>/<Properties>/<StandardAttributes>`)
/// либо ТАБЛИЧНАЯ часть (`<StandardAttributes>` прямо под source-root).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdAttrsVariant {
    /// Корень вида.
    Root,
    /// Табличная часть.
    Tabular,
}

/// Декларация платформенного `standardAttributes` вида — ЧИСТЫЕ ДАННЫЕ, читаемые
/// обобщённым кодеком `formats_xml`. Один такой `&'static` на вид (+ на его табличную
/// часть) заменяет весь пер-видовой рукописный кодек (§1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdAttrsDecl {
    /// Атрибуты корня в каноническом порядке — ФИКСИРОВАННЫЙ набор (все атрибуты
    /// присутствуют всегда). Используется, когда [`root_variants`] пуст (все структурные
    /// виды с неизменным набором std-attrs: Catalog/Document/Task/…).
    ///
    /// [`root_variants`]: StdAttrsDecl::root_variants
    pub root_attrs: &'static [AttrDecl],
    /// ВАРИАДНЫЙ/УСЛОВНЫЙ набор корневых атрибутов (data-driven, §1.6): несколько
    /// АЛЬТЕРНАТИВНЫХ наборов, среди которых кодек выбирает по НАБЛЮДАЕМОМУ числу блоков
    /// (на read) / числу IR-записей (на write). Пустой `&[]` (дефолт всех прежних видов)
    /// ⇒ единственный фиксированный вариант [`root_attrs`] (поведение НЕ меняется).
    ///
    /// Механизм покрывает две формы вариативности регистров ERP:
    /// - **условный атрибут** — атрибут присутствует лишь при некотором значении keying-
    ///   поля (напр. AccumulationRegister `RecordType` — только при `registerType==Balance`):
    ///   объявляется как ДВА варианта (`[…,RecordType,…]` и `[…без RecordType…]`), различимых
    ///   по числу атрибутов;
    /// - **опциональный блок целиком** — блок std-attrs может ОТСУТСТВОВАТЬ (0 атрибутов):
    ///   объявляется вариант с пустым набором `&[]` (0 записей в IR).
    ///
    /// Наборы ОБЯЗАНЫ иметь ПОПАРНО-РАЗЛИЧНЫЕ длины (кодек дискриминирует по счётчику;
    /// неоднозначность длин — программная ошибка декларации). Наблюдаемый счётчик, не
    /// совпавший ни с одним вариантом, — типизированная §1.0-ОШИБКА (не догадка).
    ///
    /// Будущий регистр (Accounting/Calculation) объявляет СВОИ варианты ЗДЕСЬ, в своём
    /// спек-файле, БЕЗ правок общего кодека (parallel-safe fan-out, зеркало R1/R2).
    pub root_variants: &'static [&'static [AttrDecl]],
    /// Атрибуты табличной части (`LineNumber`), либо `&[]` если у вида нет табличных
    /// частей со std-attrs (DocumentJournal/ExchangePlan).
    pub tabular_attrs: &'static [AttrDecl],
    /// Variable-слоты IR-записи в EDT-порядке (типы/дефолты).
    pub var_slots: &'static [VarSlotKind],
    /// EDT-листья блока в позиционном порядке.
    pub edt_leaves: &'static [EdtLeaf],
    /// Designer-DENSE листья (25) с их local-name'ами.
    pub dense_leaves: &'static [DenseLeafRule],
    /// Local-name корневого тега-обёртки Designer (`Catalog`/`Document`/…). Для
    /// Root-варианта путь — `<root_elem>/<Properties>/<StandardAttributes>`.
    pub root_elem: &'static str,
    /// Метка вида для диагностических сообщений (`"Catalog"` и т.п.).
    pub label: &'static str,
}

impl StdAttrsDecl {
    /// Атрибуты для варианта (ФИКСИРОВАННЫЙ путь — прежние виды без вариативности).
    /// Для Root возвращает [`root_attrs`](Self::root_attrs); вариадный выбор идёт через
    /// [`root_variant_sets`](Self::root_variant_sets).
    pub const fn attrs(&self, variant: StdAttrsVariant) -> &'static [AttrDecl] {
        match variant {
            StdAttrsVariant::Root => self.root_attrs,
            StdAttrsVariant::Tabular => self.tabular_attrs,
        }
    }

    /// Является ли Root-блок ВАРИАДНЫМ (объявлены альтернативные наборы). Табличная
    /// часть всегда фиксирована.
    pub const fn root_is_variadic(&self) -> bool {
        !self.root_variants.is_empty()
    }

    /// Все допустимые наборы Root-атрибутов: вариадные (если объявлены) либо
    /// единственный фиксированный [`root_attrs`](Self::root_attrs). Кодек выбирает среди
    /// них по счётчику блоков/записей. Возвращает срез с временем жизни `&self` — кодек
    /// держит `&'static StdAttrsDecl`, поэтому элементы фактически `'static`.
    pub fn root_variant_sets(&self) -> &[&'static [AttrDecl]] {
        if self.root_is_variadic() {
            self.root_variants
        } else {
            std::slice::from_ref(&self.root_attrs)
        }
    }
}

// ============================================================================
// Декларация платформенного блока `standardTabularSections` (child-objects
// substrate; носители — ChartOfAccounts и ChartOfCalculationTypes).
//
// Стандартная ТЧ — платформенно-предопределённая табличная часть вида (CoA:
// «Виды субконто» ExtDimensionTypes; CCalcT: Leading/Displacing/BaseCalculationTypes)
// с СОБСТВЕННЫМИ задаваемыми свойствами (synonym/comment/toolTip/fillChecking) и
// ВЛОЖЕННЫМ std-attrs-блоком. Набор ТЧ у вида ФИКСИРОВАН платформой (CoA — 1,
// CCalcT — 3; ERP-witnessed оба вида × оба объекта, все три диалекта): блок либо
// опущен целиком (все ТЧ дефолтны — coverage/SSL), либо несёт ВСЕ ТЧ набора в
// каноническом порядке. Имя ТЧ в IR НЕ хранится — оно следует из позиции в
// [`StdTsDecl::sections`] (как имена атрибутов фикс-набора std-attrs); read
// СВЕРЯЕТ наблюдаемое имя с декларацией (§1.0).
//
// Канонический IR поля: `List` из записей-ТЧ (по одной на секцию, в порядке
// декларации); каждая запись — `List` из 5 значений:
//   [0] synonym      — Localized (EDT несёт ПУСТОЙ lang `<key></key>`; Designer
//                      `<v8:lang/>`; cf `{N,(lang,text)×N}` — witnessed lang="")
//   [1] comment      — Str (дефолт "")
//   [2] toolTip      — Localized (дефолт []; witnessed только пустой)
//   [3] fillChecking — Enum (дефолт DontCheck; witnessed только DontCheck)
//   [4] standardAttributes — List записей вложенного std-attrs-блока по грамматике
//       [`StdTsSection::attrs`] (та же 11-слотовая catalog-family запись, что у
//       корневого std-attrs CoA; см. VAR_SLOTS соответствующего спека).
// ============================================================================

/// ОДНА стандартная табличная часть вида: платформенное имя + декларация её
/// вложенного std-attrs-блока (фиксированный набор атрибутов в `attrs.root_attrs`;
/// грамматика листьев/слотов — как у корневых std-attrs, читается тем же
/// обобщённым кодеком `formats_xml::std_attrs_generic`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdTsSection {
    /// Платформенное имя ТЧ (`ExtDimensionTypes`, `LeadingCalculationTypes`, …) —
    /// одинаково у EDT (`<name>`) и Designer (`name="…"`); в IR не хранится.
    pub name: &'static str,
    /// Декларация ВЛОЖЕННОГО std-attrs-блока этой ТЧ (фикс-набор `root_attrs`).
    pub attrs: &'static StdAttrsDecl,
}

/// Декларация платформенного `standardTabularSections` вида — ЧИСТЫЕ ДАННЫЕ,
/// читаемые обобщённым кодеком `formats_xml::std_tabular_sections` (оба
/// XML-формата) и cf-кодеком `formats_brace::std_tabular_sections` (маркеры —
/// у cf-стороны). Один такой `&'static` на вид (§1.6: канон — в спеке вида).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdTsDecl {
    /// Фиксированный набор стандартных ТЧ в каноническом порядке (= EDT-порядок
    /// блоков = Designer-порядок `<xr:StandardTabularSection>` = cf-порядок
    /// маркеров; ERP-witnessed). Блок опционален ЦЕЛИКОМ: 0 либо все.
    pub sections: &'static [StdTsSection],
    /// Local-name корневого тега-обёртки Designer: путь блока —
    /// `<root_elem>/<Properties>/<StandardTabularSections>`.
    pub root_elem: &'static str,
    /// Метка вида для диагностических сообщений.
    pub label: &'static str,
}

/// Выравнивание (`Left` / `Center` / `Right` / `Auto`) — общий enum-примитив
/// контролов форм; канонизируется здесь, не в формате.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Align {
    /// `Auto`
    Auto,
    /// `Left`
    Left,
    /// `Center`
    Center,
    /// `Right`
    Right,
}

impl Align {
    /// Каноническое имя литерала.
    pub fn token(self) -> &'static str {
        match self {
            Align::Auto => "Auto",
            Align::Left => "Left",
            Align::Center => "Center",
            Align::Right => "Right",
        }
    }
}
