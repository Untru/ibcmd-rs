//! Локус + кодек: ГДЕ свойство лежит в XML-дескрипторе и КАК оно кодируется.
//!
//! Это «проекционная» половина XML-формата (ARCHITECTURE.md §1.4/§1.6): имена
//! тегов/атрибутов/ns живут ТОЛЬКО здесь, а канонический `id`/`value_kind`/порядок/
//! нормализация — в `core/spec`. Оба XML-формата (EDT `.mdo`, Designer `.xml`)
//! описываются ОДНОЙ парой таблиц `FieldId → (XmlLocus, Codec)` + envelope.
//!
//! Оба enum'а РАСШИРЯЕМЫ: новые виды локусов/кодеков добавляются вариантами, не
//! ломая существующие проекции (Designer-варианты `BoolText`/`LocalizedV8` уже
//! заведены под F2b-2b, хотя сам Designer-коннектор — позже).

/// ГДЕ значение свойства лежит в дескрипторе объекта.
///
/// Расширяемо: добавление формата/кейса = новый вариант, существующие проекции не
/// трогаются (§1.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlLocus {
    /// Атрибут КОРНЕВОГО элемента объекта (например, EDT `uuid` — хотя uuid живёт в
    /// идентичности объекта, не в спеке; вариант общий и пригодится Designer'у).
    RootAttr {
        /// Имя атрибута (local-name; ns корня подразумевается).
        name: &'static str,
    },
    /// Дочерний элемент(ы) на пути `path` от корня. Для F2b — путь длины 1 у плоских
    /// свойств (`<server>`) и длины 2 у вложенных (`synonym/key`). `ns` — префикс
    /// тега (пустой = без префикса, как у всех EDT-детей).
    PropElement {
        /// Путь элементов от корня (local-names), напр. `["server"]` или
        /// `["synonym"]` (контейнер локализованной строки).
        path: &'static [&'static str],
        /// Префикс пространства имён тега (`""` = без префикса).
        ns: &'static str,
    },
}

/// Диалект платформенного блока `standardAttributes`/`StandardAttributes` (§3.3
///). Блок — НЕ child-коллекция и НЕ пользовательские
/// данные: это платформенно-предопределённый, для вида `Enum` ПОБАЙТОВО-КОНСТАНТНЫЙ
/// регион (атрибуты `Order`+`Ref`, проверено: 1 вариант × 104 объекта в обоих
/// форматах, без uuid, без пер-объектной вариации). Кодек `Codec::EnumStandardAttributes`
/// эмитит/клеймит ИМЕННО этот константный блок (byte-exact), а канонический IR несёт
/// лишь presence-маркер `Bool(true)` — одинаковый у edt и designer (X by construction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdAttrsDialect {
    /// EDT: два подряд `<standardAttributes>`-блока (Order, затем Ref).
    Edt,
    /// Designer: один `<StandardAttributes>` с двумя `<xr:StandardAttribute>` (Order, Ref).
    Designer,
}

/// КАК текст/присутствие ячейки кодирует значение свойства.
///
/// Расширяемо. F2b-2a реально использует `BoolPresence`, `EnumText`, `PlainText`,
/// `LocalizedKeyVal`; `BoolText`/`LocalizedV8` заведены под Designer (F2b-2b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Codec {
    /// EDT-bool: ПРИСУТСТВИЕ элемента ⇒ `true`, отсутствие ⇒ `false`. Текст внутри
    /// при записи всегда `true` (проверяется на чтении: иной текст → ошибка).
    BoolPresence,
    /// Designer-bool: текст элемента — литерал `true`/`false` (F2b-2b).
    BoolText,
    /// Перечислимый литерал = текст элемента (канонический литерал из спека).
    EnumText,
    /// РАЗРЕЖЁННЫЙ перечислимый литерал: как [`Codec::EnumText`], но пустой токен —
    /// «отсутствует»-маркер, при котором узел НЕ эмитится ДАЖЕ плотным (Designer) райтером.
    /// Для свойств, которые ОБА формата несут лишь условно (extension-root `objectBelonging`/
    /// `configurationExtensionPurpose`: witnessed ТОЛЬКО на корне расширения .cfe; обычный
    /// dense-корень s1..s12 узла НЕ несёт). Дефолт поля в спеке — `Enum("")`. На чтении
    /// present-узел ОБЯЗАН нести непустой литерал (§1.0: present-empty неотличим от
    /// absent-маркера → ошибка, не догадка).
    EnumSparse,
    /// Целое (`PropertyValue::Int`) = текст элемента — десятичный литерал i64 без
    /// лидирующих нулей/пробелов (напр. `<sessionMaxAge>20`). На чтении строго
    /// парсится (иная форма → ошибка, §1.0: не best-effort); на записи — `to_string`
    /// (каноничен — round-trip byte-exact для нормализованных десятичных значений).
    IntText,
    /// Свободный текст = текст элемента (с XML-экранированием при записи).
    PlainText,
    /// EDT-локализация: контейнер `synonym` с парами `<key>lang</key><value>text</value>`.
    /// Несколько пар = несколько идущих подряд `key`/`value` (в корпусе F2b — одна).
    LocalizedKeyVal,
    /// Designer-локализация: `v8:item/(v8:lang, v8:content)` (F2b-2b).
    LocalizedV8,
    /// Описание типа 1С (`Type`/`ОписаниеТипов`): контейнерный кодек с зависящей-от-
    /// диалекта канонизацией type-id и плотностью квалификаторов. Хост-имя/ns берутся
    /// из локуса (переиспользуемо под `valueType` и пр.), детей разбирает
    /// [`crate::type_codec`].
    Type(crate::type_codec::TypeDialect),
    /// Nullable-скаляр с xsi-типизацией ([`crate::ir`]-`Value`/[`ValueSpec`]):
    /// `MinValue`/`MaxValue` и пр. Контейнерный кодек с зависящей-от-диалекта
    /// кодировкой xsi-типа (EDT `core:`-prefixed + `<value>`-ребёнок; Designer
    /// `xs:`-prefixed/`xsi:nil` + текст). Хост-имя/ns — из локуса; xsi-атрибут и скаляр
    /// разбирает [`crate::value_codec`]. §1.0: незнакомый xsi-тип → ОШИБКА.
    ///
    /// [`ValueSpec`]: morph1c_core::ir::value::ValueSpec
    Value(crate::value_codec::ValueDialect),
    /// Двухуровневое стиль-значение шрифта/цвета (`StyleItem.value`,
    /// [`crate::ir`]-`StyleValue`/[`StyleValueSpec`]): EDT — вложенный `<value
    /// xsi:type="core:FontValue"><value xsi:type="core:FontRef|FontDef">…`; Designer —
    /// плоский `<Value xsi:type="v8ui:Font" …/>` / `<Value xsi:type="v8ui:Color">…</Value>`.
    /// Хост-имя/ns — из локуса; xsi-типы, sub-поля и ref-канонизацию разбирает
    /// [`crate::style_value_codec`]. §1.0: незнакомый xsi-тип/sub-поле/атрибут → ОШИБКА.
    ///
    /// [`StyleValueSpec`]: morph1c_core::ir::value::StyleValueSpec
    StyleValue(crate::style_value_codec::StyleValueDialect),
    /// ОБОБЩЁННЫЙ (data-driven) вариативный платформенный блок `standardAttributes`/
    /// `StandardAttributes` структурного вида (Catalog/Document/DocumentJournal/Task/
    /// BusinessProcess/ChartOfCharacteristicTypes/ExchangePlan). Читает `&'static
    /// StdAttrsDecl` из канонического спека (`core/spec`) и проецирует его в оба
    /// XML-формата ОДНОЙ машинерией ([`crate::std_attrs_generic`]) — заменяет закрытый
    /// набор пер-видовых кодеков (§1.6: канон — в спеке, формат — лишь проекция; новый
    /// структурный вид добавляет ZERO правок в этот файл). Декодит от КОРНЯ (EDT —
    /// inline-блоки; Designer — обёртка под `<decl.root_elem>/<Properties>`). IR — `List`
    /// из N×`List` из variable-значений. Эмиссия даёт НЕСКОЛЬКО узлов.
    StdAttrs {
        /// Диалект (EDT inline / Designer wrapper).
        dialect: crate::std_attrs_generic::GenDialect,
        /// Декларация вида — чистые данные канонического спека.
        decl: &'static morph1c_core::spec::common::StdAttrsDecl,
        /// Вариант (корень / табличная часть).
        variant: morph1c_core::spec::common::StdAttrsVariant,
    },
    /// ОБОБЩЁННЫЙ (data-driven) платформенный блок `standardTabularSections`/
    /// `StandardTabularSections` — субстрат стандартных ТЧ (ChartOfAccounts
    /// `ExtDimensionTypes`; ChartOfCalculationTypes Leading/Displacing/
    /// BaseCalculationTypes). Читает `&'static StdTsDecl` из канонического спека и
    /// проецирует его в оба XML-формата одной машинерией
    /// ([`crate::std_tabular_sections`]; атрибут-уровень — вербатим
    /// [`crate::std_attrs_generic`]). Декодит от КОРНЯ (EDT — inline-блоки; Designer —
    /// обёртка под `<decl.root_elem>/<Properties>`). IR — `List` записей-ТЧ
    /// `List([synonym, comment, toolTip, fillChecking, std-attrs-List])`; блок
    /// опционален ЦЕЛИКОМ (пустой `List` ⇒ ноль узлов). Эмиссия EDT даёт НЕСКОЛЬКО
    /// узлов.
    StdTabularSections {
        /// Диалект (EDT inline / Designer wrapper).
        dialect: crate::std_attrs_generic::GenDialect,
        /// Декларация вида — чистые данные канонического спека.
        decl: &'static morph1c_core::spec::common::StdTsDecl,
    },
    /// ВАРИАТИВНЫЙ платформенный блок `standardAttributes` видов с ПРЕДОПРЕДЕЛЁННЫМ набором
    /// стандартных атрибутов — `InformationRegister` (Active/LineNumber/Recorder/Period) и
    /// `Enum` (Order/Ref): набор задаётся декларацией-данными [`IrStdAttrsDecl`], машинерия
    /// одна. Пер-объектны `synonym`/`toolTip`/`fillValue`/`fullTextSearch` (последний РАНЬШЕ
    /// был зашит константой `Use` — см. шапку [`crate::std_attrs_ir`]). Декодит от КОРНЯ
    /// (EDT — N inline-блоков; Designer — `<StandardAttributes>` под
    /// `<root_elem>/<Properties>`). IR — `List` из N×`List([syn,tip,fill,fts])`. Эмиссия
    /// даёт НЕСКОЛЬКО узлов (EDT: N блоков).
    ///
    /// [`IrStdAttrsDecl`]: crate::std_attrs_ir::IrStdAttrsDecl
    IrStandardAttributes(StdAttrsDialect, &'static crate::std_attrs_ir::IrStdAttrsDecl),
    /// `choiceParameterLinks`/`ChoiceParameterLinks` — список связей параметров выбора
    /// (дети Resource/Dimension/Attribute). Хост-имя/ns — из локуса; структуру разбирает
    /// [`crate::choice_param_links`]. IR — `List` из `List([name,field])`; пустой → `[]`.
    ChoiceParameterLinks(crate::choice_param_links::LinksDialect),
    /// EDT-only КОНСТАНТНЫЙ блок `<help><pages><lang>ru</lang></pages></help>` корня
    /// InformationRegister (20 объектов). Присутствие ⇒ `Bool(true)`; на записи (EDT)
    /// эмитит фикс-блок. Designer его НЕ проецирует (нет аналога) → поле помечено
    /// `x_ignore` в спеке (исключено из X-сравнения, см. `morph1c_testkit`). Локус даёт
    /// имя тега (`help`); структура фиксирована (byte-exact).
    HelpConst,
    /// EDT-only КОНСТАНТА `usePurposes` стаба формы: РОВНО два подряд
    /// `<usePurposes>PersonalComputer</usePurposes><usePurposes>MobileDevice</usePurposes>`
    /// (сверено 185/185). Навигирует от КОРНЯ источника (props_root формы), как
    /// standardAttributes; присутствие ⇒ `Bool(true)`, эмиссия — 2 узла. Только EDT
    /// FormRef-стаб; Designer FormRef — bare-ссылка (не проецирует это поле).
    UsePurposesConst,
    /// Designer-КОНСТАНТА `<UsePurposes>` ВЛОЖЕННОГО дескриптора формы (`Forms/<Имя>.xml`):
    /// РОВНО два `<v8:Value xsi:type="app:ApplicationUsePurpose">PlatformApplication|
    /// MobilePlatformApplication` (сверено: 18/18 coverage/s15 + 771/772 SSL; единственное
    /// отклонение — одиночный PlatformApplication — громкий отказ, §1.0). Присутствие ⇒
    /// `Bool(true)` — ТОТ ЖЕ presence-канон, что EDT [`Codec::UsePurposesConst`], поэтому
    /// designer-обогащённый FormRef-ребёнок X-равен EDT-стабу. Эмиссия — const-блок.
    FormUsePurposesV8Const,
    /// EDT-only `content` корня `ExchangePlan` (состав плана обмена; Designer `.cf` его НЕ
    /// несёт). X-исключён (`x_ignore`), но УЧАСТВУЕТ в R EDT. Декодит от КОРНЯ
    /// (multi-sibling `<content><mdObject>Path</mdObject>[<autoRecord>]…`). IR — `List` из
    /// `List([Str(mdObject), Enum(autoRecord)])`. См. [`crate::exchange_plan_content`].
    ExchangePlanContent,
    /// `content`/`Content` корня `CommonAttribute` (состав общего реквизита). Двунаправлен
    /// (edt+designer, X by construction). EDT — multi-sibling `<content><metadata>Path
    /// </metadata><use>Mode</use></content>` от КОРНЯ; Designer — single-container `<Content>`
    /// с `<xr:Item>`'ами (`<xr:Metadata>`/`<xr:Use>`/пустой `<xr:ConditionalSeparation/>`).
    /// IR — `List` из `List([Str(metadata), Enum(use)])`; пустой → `[]`. См.
    /// [`crate::common_attribute_content`].
    CommonAttributeContent(crate::common_attribute_content::CommonAttributeContentDialect),
    /// Список строк-ссылок корня (`inputByString`/`dataLockFields`/`basedOn`/`owners`).
    /// EDT — сиблинги `<tag>Path</tag>` от КОРНЯ (multi-node); Designer — контейнер с
    /// `<xr:Field>`/`<xr:Item>` (field/item-style). IR — `List([Str…])`. См.
    /// [`crate::ref_list`].
    RefList(crate::ref_list::RefListDialect),
    /// `characteristics`/`Characteristics` корня `Catalog` — список 12-полевых связок
    /// характеристик. EDT — сиблинги `<characteristics>` (12 плоских листов); Designer —
    /// контейнер `<Characteristics>` с `<xr:Characteristic>`'ами (две группы). IR —
    /// `List` записей по 12 значений. См. [`crate::characteristics`].
    Characteristics(crate::characteristics::CharacteristicsDialect),
    /// `<predefined>` корня `Catalog` (предопределённые данные) — ИНЛАЙН-локус EDT. Designer
    /// тело НЕСЁТ, но САЙДКАРОМ `<Name>/Ext/Predefined.xml` (его читает
    /// `morph1c_pipeline::predefined_read` в ЭТО ЖЕ спек-поле → оба диалекта дают РАВНЫЙ IR).
    /// Поэтому в Designer-LocusMap поле не проецируется, X-исключён (`x_ignore`), но
    /// УЧАСТВУЕТ в R EDT. Декодит от КОРНЯ (рекурсивные `<items>`/`<content>`).
    /// См. [`crate::predefined`].
    PredefinedData,
    /// `<predefined>` корня `ChartOfCharacteristicTypes` (вариант [`PredefinedData`] с
    /// ДОПОЛНИТЕЛЬНЫМ `<type>`-узлом — предопределённые виды характеристик несут
    /// ОписаниеТипов значения; `<code>`/`<isFolder>`/`<content>` у CCT ТОЖЕ витнессированы
    /// (ERP: коды `АналитикиСтатейБюджетов`, папки/рекурсия `СтатьиАктивовПассивов`), но
    /// `<isFolder>` идёт ПЕРЕД `<code>`, а `<code>` — плоский текст-лист, не Value-xsi).
    /// Как и [`PredefinedData`]: EDT — инлайн, Designer — сайдкар `Ext/Predefined.xml`.
    /// Декодит от КОРНЯ. См. [`crate::predefined_cct`].
    PredefinedDataCct,
    /// `<predefined>` корня `ChartOfAccounts` (предопределённые счета) — СВОЯ грамматика
    /// (≠ Catalog/CCT): plain `<code>`, `<accountType>` (литерал, деф. Active),
    /// `<offBalance>`, `<order>` со ЗНАЧИМЫМИ ведущими пробелами, ссылки
    /// `<accountingFlags>`, блоки `<extDimensionTypes>` (characteristicType/turnover/
    /// extDimensionAccountingFlags), рекурсивные `<childItems>`. ERP-witnessed
    /// (Хозрасчетный 438 + Международный 1). Как и [`PredefinedData`]: EDT — инлайн,
    /// Designer — сайдкар `Ext/Predefined.xml`. Декодит от КОРНЯ. См.
    /// [`crate::predefined_coa`].
    PredefinedDataCoa,
    /// `picture`/`Picture` команды (ссылка на стандартную картинку). EDT —
    /// `<picture xsi:type="core:PictureRef"><picture>ref</picture></picture>`; Designer —
    /// `<Picture><xr:Ref>ref</xr:Ref><xr:LoadTransparent>true</…></Picture>`. IR — `Str`
    /// (путь; `""`=пусто). См. [`crate::picture`].
    PictureRef(crate::picture::PictureDialect),
    /// EDT-узел `<transparentPixel><x>N</x><y>M</y></transparentPixel>` вида `CommonPicture`
    /// (координата прозрачного пикселя, metamodel `Point`; SPARSE-листья — нулевая
    /// координата опускается, witnessed ERP 30/2458). Designer НЕ несёт его в дескрипторе
    /// (носитель — обёртка сайдкара `Ext/Picture.xml`, читает/пишет
    /// `morph1c_pipeline::picture_read` в то же спек-поле; поле `x_ignore`). IR —
    /// `List([Int(x), Int(y)])`; пустой `List` = «пикселя нет» (дефолт). См.
    /// [`crate::transparent_pixel`].
    TransparentPixel,
    /// `shortcut`/`Shortcut` command datatype `Shortcut` (key combination
    /// `Ctrl+S`/`F3`/…). BOTH formats carry it as leaf TEXT, differing only in the tag
    /// name (locus): EDT `<shortcut>Ctrl+S</shortcut>` (empty → omitted); Designer
    /// `<Shortcut>Ctrl+S</Shortcut>` (empty → `<Shortcut/>`). IR — `Str` (`""`=empty).
    /// A named value-kind (not `PlainText`): any field of any kind declares
    /// `Codec::Shortcut` in its projection with no per-parent hardcode (data-driven).
    /// See [`crate::shortcut`].
    Shortcut,
    /// `linkByType`/`LinkByType` реквизита (связь по типу). EDT —
    /// `<linkByType><field>Path</field></linkByType>`; Designer — `<LinkByType><xr:DataPath>
    /// Path</xr:DataPath><xr:LinkItem>0</…></LinkByType>`. IR — `Str` (`""`=пусто). См.
    /// [`crate::link_by_type`].
    LinkByType(crate::link_by_type::LinkByTypeDialect),
    /// `choiceParameters`/`ChoiceParameters` реквизита (параметры выбора). Список связок
    /// `(name, value-или-массив)`. EDT — сиблинги от КОРНЯ (multi-node); Designer —
    /// контейнер с `<app:item>`'ами. IR — `List([List([name,kind,values])])`. См.
    /// [`crate::choice_parameters`].
    ChoiceParameters(crate::choice_parameters::ChoiceParametersDialect),
    /// `containedObjects` корня `Configuration` (платформенные `(classId, objectId)` пары).
    /// EDT — сиблинги `<containedObjects classId objectId/>` от КОРНЯ (multi-node); Designer
    /// — `<InternalInfo><xr:ContainedObject><xr:ClassId/><xr:ObjectId/></xr:ContainedObject>…`
    /// от КОРНЯ `<Configuration>`. IR — `List([List([Str,Str])])`. См. [`crate::configuration`].
    ContainedObjects(crate::configuration::ConfigDialect),
    /// `<ChildObjects>` корня `Configuration` — ИМЕНА всех объектов конфигурации
    /// (членство+порядок, НЕ рекурсия). EDT — сиблинги `<plural>Kind.Name</plural>` от КОРНЯ
    /// (+ Language синтез. из inline `<languages>`); Designer — `<ChildObjects><Kind>Name
    /// </Kind>…` от КОРНЯ. IR — `List([List([Str(kind),Str(name)])])`. См. [`crate::configuration`].
    ConfigChildObjects(crate::configuration::ConfigDialect),
    /// EDT-only inline-сущности `<languages uuid><name><synonym><languageCode>` корня
    /// `Configuration`. Designer тело языка хранит в отдельном файле → НЕ проецирует это
    /// поле (поле `x_ignore`, R EDT-only). От КОРНЯ (multi-node). IR — `List([List([uuid,
    /// name,langCode,synonym-pairs])])`. См. [`crate::configuration`].
    LanguagesEntity,
    /// Designer `<UsePurposes><v8:Value xsi:type="app:ApplicationUsePurpose">X</v8:Value>…`.
    /// Host claimed `locate`'ом. IR — `List([Str])`. (EDT — plain `EnumText`; значения
    /// расходятся → поле `x_ignore`.) См. [`crate::configuration`].
    UsePurposesV8,
    /// `usedMobileApplicationFunctionalities`/`UsedMobileApplicationFunctionalities` корня
    /// `Configuration` (вложенный список `(functionality, use)`). Host claimed `locate`'ом;
    /// `app_ns` внутренних тегов — из диалекта (`""` EDT / `"app"` Designer). EDT sparse vs
    /// Designer dense РАСХОДЯТСЯ → поле `x_ignore` (R обоих byte-exact). IR — `List([List([
    /// Str,Bool])])`. См. [`crate::configuration`].
    MobileFunctionalities(crate::configuration::ConfigDialect),
    /// XDTO-type-ref — ССЫЛКА на XDTO-тип как пара `(name, nsUri)` (WebService.Operation
    /// `xdtoReturningValueType` / `.Parameter.xdtoValueType`). Host-имя/ns — из локуса;
    /// EDT — `<host><name>…</name><nsUri>…</nsUri></host>`; Designer — QName-лист
    /// `prefix:local` (+опц. локальный `xmlns:d6p1`). IR — `List([Str(name), Str(nsUri)])`.
    /// См. [`crate::xdto_type_ref`].
    XdtoTypeRef(crate::xdto_type_ref::XdtoTypeRefDialect),
    /// `xdtoPackages`/`XDTOPackages` корня `WebService` — СПИСОК типизированных
    /// (`String`|`Reference`) ссылок на XDTO-пакеты. EDT — сиблинги `<xdtoPackages
    /// xsi:type="core:StringValue|core:ReferenceValue"><value>…` от КОРНЯ (multi-node);
    /// Designer — контейнер `<XDTOPackages>` с `<xr:Item>`'ами. IR — `List([List([
    /// Str(variant), Str(value)])])`; пустой → `[]`. См. [`crate::xdto_packages`].
    XdtoPackages(crate::xdto_packages::XdtoPackagesDialect),
    /// ПУСТОЙ типизированный `ValueList`-элемент (`<Tag xsi:type="xr:ValueList"/>`) — Designer-
    /// DENSE проекция всегда-пустого списочного свойства (`WebSocketClient.headers`). Хост-имя/ns
    /// — из локуса; xsi-тип фиксирован (`xr:ValueList`). IR — `List` (в корпусе всегда `[]`).
    /// §1.0 witnessed-only: read СВЕРЯЕТ пустоту (нет детей/текста, xsi:type == `xr:ValueList`) и
    /// даёт `List([])`; НЕпустой/иной xsi-тип → ОШИБКА (не угадывает layout непустого списка).
    /// EDT его НЕ проецирует (в `.mdo` нет `<headers>` — пустой список опущен); cf — константа
    /// `{0}` (hand-written). Так все три формата дают ПУСТОЙ `List` → сжатие в дефолт `[]`
    /// (X by construction). Появится непустой ValueList — расширить кодек структурно.
    EmptyValueList,
    /// `compatibilityMode`/`CompatibilityMode` корня `Configuration` — enum режима совместимости
    /// с РАСХОДЯЩЕЙСЯ пер-форматной кодировкой: EDT dotted `8.3.20`, Designer prefixed
    /// `Version8_3_20`. Канон IR = EDT-dotted (`8.3.20`); EDT-коннектор пишет его verbatim
    /// (`EnumText`), а Designer-коннектор ТРАНСЛИРУЕТ read (`Version8_3_20`→`8.3.20`) и write
    /// (`8.3.20`→`Version8_3_20`) — см. `compat_designer_to_canonical`/`compat_canonical_to_designer`
    /// в `lib.rs`. Нужен для КРОСС-форматной конвертации (edt→designer): без трансляции
    /// Designer-writer эмитил бы `8.3.20`, которое платформа отвергает («Неверное значение
    /// перечисления»). Значение X-исключено в спеке (обе стороны канонятся к одному — трансляция
    /// делает их равными, но x_ignore сохранён для устойчивости). Text-лист (claim_with_text).
    CompatibilityMode,
    /// EDT-only структурный узел `<extension xsi:type="mdclassExtension:ConfigurationExtension">`
    /// корня расширения (.cfe): список листьев-флагов `<имяСвойства>Checked</имяСвойства>`
    /// (заимствованные свойства). Designer аналога в дескрипторе не несёт (поле `x_ignore`).
    /// IR — `List([Str(имя-флага)…])`. §1.0: иной xsi:type / текст флага ≠ `Checked` /
    /// атрибуты/дети у флага → ОШИБКА (witnessed-only форма). См. [`crate::configuration`].
    ExtensionFlags,
}

/// Где лежит ДОЧЕРНЯЯ КОЛЛЕКЦИЯ в дескрипторе и как фреймится идентичность ребёнка
/// (child-objects substrate). Форматно-нейтральный
/// `ChildSlot.collection` отображается ПРОЕКЦИЕЙ в этот локус (имена тегов — здесь).
///
/// * EDT: дети — прямые элементы корня с тегом `child_tag` (`<enumValues>`); путь
///   `container` пуст; идентичность плоская (`@uuid` + лист `name`).
/// * Designer: дети — внутри обёртки `container` (`["Enum","ChildObjects"]`) с тегом
///   `child_tag` (`<EnumValue>`); идентичность в `<Properties>` (`@uuid` + `<Name>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildLocus {
    /// Путь от корня объекта до КОНТЕЙНЕРА коллекции (local-names). Пуст ⇒ дети —
    /// прямые элементы корня (EDT). Designer: `["Enum","ChildObjects"]`.
    pub container: &'static [&'static str],
    /// Local-name тега одного элемента коллекции (`"enumValues"` / `"EnumValue"`).
    pub child_tag: &'static str,
    /// Завёрнута ли идентичность/свойства ребёнка в `<Properties>` (Designer) или
    /// лежат плоско прямо под элементом коллекции (EDT).
    pub props_wrapped: bool,
    /// Local-name тега-листа имени ребёнка (`"name"` EDT / `"Name"` Designer).
    pub name_tag: &'static str,
    /// BARE-REF коллекция (§3.2 child-objects-design): элемент коллекции — лишь ИМЯ-
    /// ссылка `<Form>Имя</Form>` БЕЗ `@uuid`/`<Properties>`/свойств (Designer Form/
    /// Template). Тело под-объекта — в отдельном файле; в дескрипторе родителя живёт
    /// только членство+порядок. Read: `uuid=zero`, `properties=[]`; Write: `<child_tag>
    /// Имя</child_tag>`. EDT-сторона той же коллекции НЕ bare (несёт полный inline-стаб
    /// uuid+name+props) → её локус задаёт `bare_ref=false`. X сравнивает такие коллекции
    /// по ИМЕНИ+ПОРЯДКУ (X-exception, см. `morph1c_testkit`), т.к. EDT-стаб несёт
    /// денормализованные form-метаданные, отсутствующие у Designer-ссылки.
    pub bare_ref: bool,
}

/// Одна строка проекции: канонический локус + кодек одного поля.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProjection {
    /// Где лежит ячейка поля.
    pub locus: XmlLocus,
    /// Как ячейка кодирует значение.
    pub codec: Codec,
}

impl FieldProjection {
    /// Сконструировать строку проекции.
    pub const fn new(locus: XmlLocus, codec: Codec) -> Self {
        FieldProjection { locus, codec }
    }
}
