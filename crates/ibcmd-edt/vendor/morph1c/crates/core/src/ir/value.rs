//! Значения свойств IR — модель [`PropertyValue`] (ARCHITECTURE.md §1.3) и её
//! опорные типы (`Token`, `Lang`, `TypeSpec`, `ValueSpec`, `BlobRef`).
//!
//! Это семантически различимая, ФОРМАТ-НЕЙТРАЛЬНАЯ модель значения (§1.1): в ней
//! нет ни slot-номеров `.cf`, ни имён XML-тегов. Один логический объект даёт ОДИН
//! и тот же `PropertyValue` независимо от формата (§1.6) — формат лишь проецирует.
//!
//! Принцип бинарной приёмки (§1.0): варианта `Raw` НЕТ намеренно. Поле либо
//! типизировано одним из вариантов ниже, либо это [`PropertyValue::Blob`]
//! (опаковый платформенный бинарь, переносимый as-is), либо движок падает с
//! ошибкой. `Blob` — не escape-hatch для структурных данных.

use serde::{Deserialize, Serialize};

/// Значение свойства IR.
///
/// Реализовано РОВНО по §1.3: `Bool / Int / Str / Enum / Ref / Localized / Type /
/// Value / List / Blob`. **`Raw` отсутствует намеренно** (§1.0) — нет варианта
/// «структура, которую не разобрали».
///
/// Порядок вариантов фиксирован: он влияет на стабильную сериализацию (§1.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    /// Логический литерал.
    Bool(bool),
    /// Целое (число 1С, влезающее в i64 — счётчики, длины, индексы).
    Int(i64),
    /// Свободный текст.
    Str(String),
    /// Перечислимый литерал (`Auto`, `DontUse`, …) — НЕ свободный текст.
    Enum(Token),
    /// Ссылка `Тип.Имя` / `Объект.Поле` (нормализуется спеком, не форматом).
    Ref(String),
    /// Локализованная строка: упорядоченный список `(язык, текст)`.
    Localized(Vec<(Lang, String)>),
    /// Описание типа 1С.
    Type(TypeSpec),
    /// Nullable-скаляр с квалификаторами (Min/Max/Fill и т.п.).
    Value(ValueSpec),
    /// Значение стиля (`StyleItem.value`) — двухуровневое xsi-типизированное значение
    /// шрифта/цвета (`FontValue/FontRef|FontDef`, `ColorValue/ColorRef|ColorDef`). См.
    /// [`StyleValueSpec`].
    StyleValue(StyleValueSpec),
    /// Однородный/разнородный список значений (сохраняет порядок).
    List(Vec<PropertyValue>),
    /// ТОЛЬКО опаковый платформенный бинарь (макеты MXL / картинки /
    /// скомпилированные модули) — переносится as-is, по ссылке (§1.0, §1.3).
    Blob(BlobRef),
}

impl PropertyValue {
    /// Стабильный дискриминант вида значения — для проверки `value_kind` спека
    /// (§1.6: тип значения задан спеком, а не форматом) без `match` по всему
    /// значению. НЕ зависит от полезной нагрузки.
    pub fn kind(&self) -> ValueKind {
        match self {
            PropertyValue::Bool(_) => ValueKind::Bool,
            PropertyValue::Int(_) => ValueKind::Int,
            PropertyValue::Str(_) => ValueKind::Str,
            PropertyValue::Enum(_) => ValueKind::Enum,
            PropertyValue::Ref(_) => ValueKind::Ref,
            PropertyValue::Localized(_) => ValueKind::Localized,
            PropertyValue::Type(_) => ValueKind::Type,
            PropertyValue::Value(_) => ValueKind::Value,
            PropertyValue::StyleValue(_) => ValueKind::StyleValue,
            PropertyValue::List(_) => ValueKind::List,
            PropertyValue::Blob(_) => ValueKind::Blob,
        }
    }
}

/// Стабильный дискриминант [`PropertyValue`] без полезной нагрузки.
///
/// Спек объявляет `value_kind` каждого поля ОДИН раз (§1.6); движок проверяет, что
/// декодированное форматом значение соответствует объявленному виду, — так формат
/// лишён свободы решить тип сам (анти-дивергенция).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueKind {
    /// см. [`PropertyValue::Bool`]
    Bool,
    /// см. [`PropertyValue::Int`]
    Int,
    /// см. [`PropertyValue::Str`]
    Str,
    /// см. [`PropertyValue::Enum`]
    Enum,
    /// см. [`PropertyValue::Ref`]
    Ref,
    /// см. [`PropertyValue::Localized`]
    Localized,
    /// см. [`PropertyValue::Type`]
    Type,
    /// см. [`PropertyValue::Value`]
    Value,
    /// см. [`PropertyValue::StyleValue`]
    StyleValue,
    /// см. [`PropertyValue::List`]
    List,
    /// см. [`PropertyValue::Blob`]
    Blob,
}

/// Перечислимый литерал — имя члена перечисления-свойства (`Auto`, `DontUse`, …).
///
/// Отдельный тип (а не `String`), чтобы [`PropertyValue::Enum`] нельзя было спутать
/// со свободным текстом [`PropertyValue::Str`]: это разные виды значения (§1.6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token(pub String);

impl Token {
    /// Построить токен из любого строкоподобного.
    pub fn new(s: impl Into<String>) -> Self {
        Token(s.into())
    }
    /// Литерал как `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Язык локализованной строки — код языка 1С (`ru`, `en`, …).
///
/// Хранится как короткий код; нормализация (lower-case и т.п.) — забота спека, а
/// не формата, чтобы оба формата дали один [`PropertyValue::Localized`] (§1.6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Lang(pub String);

impl Lang {
    /// Построить язык из кода.
    pub fn new(code: impl Into<String>) -> Self {
        Lang(code.into())
    }
    /// Код языка как `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Описание типа 1С (`Type` в §1.3).
///
/// Упорядоченный НАБОР компонент-типов; у каждой ПРИМИТИВНОЙ компоненты — опциональный
/// квалификатор своего вида (string/number/date/binary). Порядок набора —
/// authored order, **СОХРАНЯЕТСЯ** (НЕ сортируется): edt и designer хранят один и тот
/// же порядок (проверено 134/134), порядок — часть идентичности значения.
///
/// `present-but-empty` (`parts` пуст) — валидное состояние (типизированный реквизит
/// без типа): это ЗНАЧЕНИЕ, а не отсутствие поля. Codec/спек различают Absent (нет
/// хоста) и present-empty (`<type/>`/`<Type/>` ⇒ `parts:[]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeSpec {
    /// Компоненты набора в КАНОНИЧЕСКОМ (= authored) порядке. НЕ сортируется (§1.6).
    pub parts: Vec<TypeRef>,
}

/// Одна компонента [`TypeSpec`] — канонизированный id типа плюс опциональный
/// квалификатор, привязанный 1:1 к ЭТОЙ компоненте (проверено: 0 orphan, 0 dup в
/// 3363 type-блоках корпуса).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeRef {
    /// Канонизированный id типа в bare-EDT-форме (`Number`, `String`, `Date`,
    /// `Boolean`, `CatalogRef.Товары`, `EnumRef.X`, `AnyRef`, `ValueStorage`,
    /// `FixedStructure`, `UUID`, `ValueList`, …). Один и тот же у edt и designer
    /// после канонизации (canon↔QName таблица + xsd/ValueList алиасы в codec).
    pub id: String,
    /// Квалификатор компоненты ИЛИ `None`. `Some` ⇔ у компоненты в источнике есть
    /// свой квалификатор-блок (для `String`/`Number`/`Date` он присутствует ВСЕГДА в
    /// обоих форматах — проверено 3363/3363). Дефолтные ЛИСТЫ внутри квалификатора
    /// нормализованы (length/precision→0, fixed/nonnegative→false, fractions→DateTime),
    /// но сам квалификатор сохраняется как `Some` — это и даёт X-равенство
    /// EDT-sparse `<stringQualifiers/>` ↔ Designer-dense `Length 0 / Variable`.
    pub qualifier: Option<TypeQualifier>,
}

/// Квалификатор примитивной компоненты — РОВНО один из видов, соответствующий
/// примитиву. Типобезопасный enum (а не `Vec<(String,Value)>`): нельзя положить
/// квалификатор не того вида и нельзя случайно расхождение default-омиссии между
/// форматами — каноничность by construction (§1.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeQualifier {
    /// `String`. `fixed=false` ⇔ AllowedLength=Variable (default).
    String {
        /// Длина строки (EDT `length`/Designer `Length`); 0 = дефолт (пустой
        /// EDT-блок `<stringQualifiers/>`).
        length: u32,
        /// Фиксированная длина (EDT `fixed`/Designer AllowedLength=Fixed).
        fixed: bool,
    },
    /// `Number`. `precision` = Цифры/Digits; `scale` = ДробнаяЧасть/FractionDigits;
    /// `nonnegative` ⇔ AllowedSign=Nonnegative (false ⇔ Any).
    Number {
        /// Общее число знаков (EDT `precision`/Designer `Digits`).
        precision: u32,
        /// Знаков после запятой (EDT `scale`/Designer `FractionDigits`); 0 = дефолт.
        scale: u32,
        /// Только неотрицательные (AllowedSign=Nonnegative); false = Any (дефолт).
        nonnegative: bool,
    },
    /// `Date`. `fractions`: DateTime (default) | Date | Time.
    Date {
        /// Состав даты/времени.
        fractions: DateFractions,
    },
    /// `BinaryData` — заложен по инвентарю; в SSL не встретился. Codec ОБЯЗАН
    /// ошибаться (а не угадывать) до подтверждения дифф-фикстурой (§1.0, §6 UNKNOWNS).
    Binary {
        /// Длина (точные имена тегов — UNKNOWN до research).
        length: u32,
        /// Фиксированная длина.
        fixed: bool,
    },
}

/// Состав значения даты (`dateFractions`/`DateFractions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DateFractions {
    /// Дата и время (default — EDT опускает, эмитя пустой `<dateQualifiers/>`).
    DateTime,
    /// Только дата.
    Date,
    /// Только время.
    Time,
}

/// Nullable-скаляр с квалификаторами (`Value` в §1.3): Min/Max/Fill-подобные
/// значения, где «значение отсутствует» — самостоятельное состояние.
///
/// Несёт ОБА: внутренний скаляр (`scalar`) И его xsi-вид (`kind`). Вид нужен, потому
/// что один и тот же `PropertyValue::Str("0")` логически может быть строкой ИЛИ датой
/// (`core:DateValue 0001-01-01T00:00:00`), а `Int(0)` — числом; xsi-тип на write
/// детерминируется `kind`, а не догадкой по нагрузке (§1.0). Подтверждено корпусом:
/// EDT `core:{Undefined,String,Boolean,Number,Date,Reference}Value` ↔ Designer
/// `xsi:nil` / `xs:{string,boolean,decimal,dateTime}` / `xr:DesignTimeRef`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueSpec {
    /// xsi-вид значения (определяет кодировку обоих форматов). `Undefined` ⇔
    /// `scalar==None`; прочие виды несут соответствующий `scalar`.
    #[serde(default)]
    pub kind: ValueScalarKind,
    /// Внутренний скаляр; `None` = явно «значение не задано» (не дефолт-омиссия,
    /// а смысловое отсутствие, при `kind==Undefined`).
    pub scalar: Option<Box<PropertyValue>>,
}

/// xsi-вид скаляра [`ValueSpec`] — формат-нейтральный канон (один у edt и designer).
///
/// Каждый вид ⇒ ОДНА пара edt/designer xsi-кодировок (см. `value_codec`). Домен — ровно
/// засвидетельствованные корпусом формы (§1.0: новый xsi-тип → ошибка, не догадка).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ValueScalarKind {
    /// Значение не задано (`core:UndefinedValue` / `xsi:nil="true"`). `scalar==None`.
    #[default]
    Undefined,
    /// Строка (`core:StringValue` / `xs:string`). `scalar = Str`.
    Str,
    /// Логическое (`core:BooleanValue` / `xs:boolean`). `scalar = Bool`.
    Bool,
    /// Число (`core:NumberValue` / `xs:decimal`). `scalar = Str` (десятичный литерал
    /// как есть — `"0"`, `"3600"`; хранится текстом, чтобы round-trip был byte-exact
    /// без догадок о форме числа).
    Number,
    /// Дата/время (`core:DateValue` / `xs:dateTime`). `scalar = Str` (ISO-литерал
    /// `"0001-01-01T00:00:00"`).
    Date,
    /// Ссылка времени конструирования (`core:ReferenceValue` / `xr:DesignTimeRef`).
    /// `scalar = Str` (`"Catalog.X.EmptyRef"`); пустая ссылка → `scalar=Str("")`.
    Reference,
    /// Пустое ОписаниеТипов (`core:TypeDescriptionValue` с пустым `<value/>` / Designer
    /// `v8:TypeDescription` self-close). Дефолтное `fillValue` атрибута `ValueType` плана
    /// видов характеристик. `scalar==None` (несёт лишь маркер вида; непустые
    /// TypeDescription в корпусе fillValue не встречены). См. `formats-xml/value_codec`.
    TypeDescription,
    /// Вид счёта (`form:AccountTypeValue` с `<value>ЛИТЕРАЛ</value>` / Designer
    /// `ent:AccountType` текст-литерал / cf `{"#",872f7198-…,N}`). `fillValue` std-атрибута
    /// `Type` плана счетов (ERP-witnessed Хозрасчетный). `scalar = Str` — литерал enum'а
    /// `Active`/`Passive`/`ActivePassive` как есть (cf-коды 0/1/2 — witnessed корреляцией
    /// 438/438 предопределённых счетов Хозрасчетного с Designer-сайдкаром).
    AccountType,
    /// Список значений (`core:ValueList` / Designer `xr:ValueList`). ПУСТОЙ типизированный
    /// маркер «Список значений» (`scalar==None`) — засвидетельствован ТОЛЬКО пустым (self-close)
    /// значением-нагрузкой записи `choiceList` (ERP-witness Document.ВходящийДокументСЭДОФСС.
    /// ФормаВыбораТребования: EDT `<value xsi:type="core:ValueList"/>` ⟺ Designer
    /// `<Value xsi:type="xr:ValueList"/>`). Непустой ValueList в корпусе НЕ встречен ⇒ кодеки
    /// ошибаются на нём (§1.0 — layout вложенных значений не витнессирован). cf-ячейка
    /// эмитится по witness-ключу (`mine`) — UNVERIFIED, не отказ (рантайм-данные).
    ValueList,
    /// Witnessed system enum scalar; stores the qualified EDT enum/member.
    SystemEnum,
}

/// Значение стиля (`StyleItem.value`, §1.3-адъюнкт) — формат-нейтральный канон
/// двухуровневого xsi-типизированного значения шрифта/цвета.
///
/// В EDT это `<value xsi:type="core:FontValue"><value xsi:type="core:FontRef|FontDef">…`
/// (mirror `ColorValue`/`ColorRef`/`ColorDef`); в Designer — плоский
/// `<Value xsi:type="v8ui:Font" ref=… …/>` либо `<Value xsi:type="v8ui:Color">#RRGGBB|pref:Name</Value>`.
/// Оба формата декодируют в ОДИН `StyleValueSpec` (X by construction, §1.6); каждый
/// кодек детерминированно восстанавливает свою байтовую конвенцию (R byte-exact).
///
/// Домен — ровно засвидетельствованный корпусом SSL (97 StyleItem, оба диалекта);
/// незнакомый xsi-тип/sub-поле/атрибут → ОШИБКА кодека (§1.0), не догадка.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StyleValueSpec {
    /// Шрифт (`type=Font`): либо ссылка на именованный шрифт-стиль (`Ref`), либо
    /// абсолютное определение по имени гарнитуры (`Def`).
    Font(FontStyle),
    /// Цвет (`type=Color`): либо ссылка на именованный цвет палитры/веб/стиля (`Ref`),
    /// либо абсолютное определение по RGB-компонентам (`Def`).
    Color(ColorStyle),
    /// Рамка (`type=Border`): абсолютное определение рамки (`BorderDef`) — тип рамки
    /// (`ControlBorderType`) + ширина. В корпусе засвидетельствован только `BorderDef`
    /// (EDT `<value xsi:type="core:BorderValue"><value xsi:type="core:BorderDef"/></value>`
    /// ⇔ Designer `<Value xsi:type="v8ui:Border" width="0"><v8ui:style
    /// xsi:type="v8ui:ControlBorderType">WithoutBorder</v8ui:style></Value>`).
    Border(BorderStyle),
}

/// Рамка стиля — `BorderDef` (абсолютное определение). `BorderRef` в корпусе не
/// засвидетельствован (кодек ошибается на нём, §1.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    /// `BorderDef` — абсолютное определение рамки: тип рамки + ширина. EDT опускает
    /// дефолты (`<style>WithoutBorder` и `<width>0` → пустой `<value
    /// xsi:type="core:BorderDef"/>`); Designer эмитит DENSE (`width="0"` + `<v8ui:style
    /// …>WithoutBorder`). Оба сходятся к одному канону.
    Def {
        /// Тип рамки (`ControlBorderType`-токен: `WithoutBorder`/`Single`/…). Дефолт
        /// `WithoutBorder` — EDT опускает `<style>`.
        style: String,
        /// Ширина рамки в пикселях. Дефолт `0` — EDT опускает `<width>`.
        width: u32,
    },
}

/// Шрифт стиля — `FontRef` (ссылка) либо `FontDef` (абсолют).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FontStyle {
    /// `FontRef` — ссылка на именованный шрифт `Style.Имя`/`System.Имя` (EDT `<font>`,
    /// Designer `ref="style:Имя"`/`ref="sys:Имя"`). КАЖДЫЙ флаг начертания — ТРИСТЕЙТ
    /// (`None` = не переопределён, наследуется от базового шрифта): ERP несёт ЧАСТИЧНЫЕ
    /// наборы (`ЖирныйПодчеркнутыйШрифт` — только bold+underline;
    /// `ПодчеркнутыйШрифт` — только underline), SSL — либо все четыре, либо ни одного.
    /// `height`/`scale` независимо опциональны (тот же тристейт: ERP
    /// `ОбычнаяГруппаШрифтБЗК` несёт ЯВНЫЙ `scale=100`).
    Ref {
        /// Канон `Style.Имя` (EDT `Style.TextLevel2` = Designer `style:TextLevel2`)
        /// либо `System.Имя` (системный шрифт, Designer `kind="WindowsFont"`).
        font: String,
        /// Высота в целых пунктах (EDT `<height>14.0</height>` = Designer `height="14"`);
        /// `None` = поле опущено.
        height: Option<u32>,
        /// Тристейт-флаги начертания (`None` = флаг не переопределён и опущен в обоих
        /// xml-диалектах; порядок эмиссии фиксирован bold→italic→underline→strikeout).
        flags: FontFlags,
        /// Масштаб в процентах (`scale`); `None` = опущен.
        scale: Option<u32>,
    },
    /// `FontDef` — абсолютное определение шрифта по имени гарнитуры + высоте (EDT
    /// `<faceName>Arial</faceName><height>14.0</height>`). Флаги начертания — ПЛОСКИЕ
    /// булевы (у абсолютного шрифта нет наследования, дефолт false): EDT эмитит только
    /// true-флаги (witnessed ERP: `ЖирныйШрифтEDI` bold, `МелкийНаклонныйШрифтБЭД` italic,
    /// `ПодчеркнутыйТекстСервисДоставки` underline, `ЗачеркнутыйШрифтБЭД` strikeout);
    /// Designer эмитит все четыре денсово (`bold="true"…`). `scale` всегда 100 (witnessed).
    Def {
        /// Имя гарнитуры (`Arial`).
        face_name: String,
        /// Высота в целых пунктах (present всегда у `FontDef` корпуса).
        height: u32,
        /// Флаги начертания (дефолт — все false).
        face: FontFace,
    },
}

/// ТРИСТЕЙТ-флаги начертания `FontRef`: `None` = флаг не переопределён (опущен в
/// источнике), `Some(v)` = переопределён значением `v`. Витнессировано ERP: присутствие
/// каждого флага независимо; порядок в обоих xml-диалектах фиксирован
/// bold→italic→underline→strikeout (пропуски не меняют порядок остальных).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFlags {
    /// Полужирный (`bold`).
    pub bold: Option<bool>,
    /// Курсив (`italic`).
    pub italic: Option<bool>,
    /// Подчёркнутый (`underline`).
    pub underline: Option<bool>,
    /// Зачёркнутый (`strikeout`).
    pub strikeout: Option<bool>,
}

/// Флаги начертания абсолютного шрифта (`FontDef`) — четыре плоских булевых
/// (дефолт false; наследования у абсолюта нет).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFace {
    /// Полужирный (`bold`).
    pub bold: bool,
    /// Курсив (`italic`).
    pub italic: bool,
    /// Подчёркнутый (`underline`).
    pub underline: bool,
    /// Зачёркнутый (`strikeout`).
    pub strikeout: bool,
}

/// Цвет стиля — `ColorRef` (ссылка) либо `ColorDef` (абсолют по RGB).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorStyle {
    /// `ColorRef` — именованный цвет `Prefix.Имя` (EDT `<color>Palette.Blue</color>` =
    /// Designer текст `pal:Blue`). Префиксы: `Palette`⇔`pal`, `Web`⇔`web`, `Style`⇔`style`.
    Ref(String),
    /// `ColorDef` — абсолютный RGB. Каждая компонента 0..=255; в EDT нулевая
    /// компонента ОПУСКАЕТСЯ (`<green>150</green><blue>70</blue>` = red 0), Designer
    /// эмитит полный `#RRGGBB` (`#009646`). Канон хранит все три (0 = «опущено»).
    Def {
        /// Красная компонента (0..=255; 0 ⇔ опущена в EDT / `00` в Designer-hex).
        red: u8,
        /// Зелёная компонента.
        green: u8,
        /// Синяя компонента.
        blue: u8,
    },
}

/// Ссылка на опаковый платформенный бинарь (`Blob` в §1.3).
///
/// Сам байтовый поток в IR НЕ материализуется (§1.1 «эффективность»): хранится
/// адресация листа-источника. Точный носитель (часть `.cf`, файл EDT) — забота
/// проекции формата; IR держит формат-нейтральную ссылку и хеш для равенства.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobRef {
    /// Формат-нейтральный логический ключ бинаря (например, имя макета/картинки).
    pub key: String,
    /// Размер полезной нагрузки в байтах (для дешёвой проверки равенства/диффа).
    pub len: u64,
    /// Стабильный хеш содержимого (для cross-format равенства без материализации).
    pub digest: [u8; 32],
}
