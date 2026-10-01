use super::*;

/// Поле схемы набора данных DCS (EDT `<fields>` ⟺ Designer `<Field>`): путь + поле + опц.
/// заголовок + опц. ограничения использования. ОБА формата несут ⇒ X-сравнимо.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsField {
    /// Ordered UH data-set field expressions; EDT orderExpressions, native orderExpression.
    #[serde(default)]
    pub order_expressions: Vec<DcsOrderExpression>,
    /// Вложенный набор данных: `true` ⇒ EDT `xsi:type="schema:DataCompositionSchemaNestedDataSet"`
    /// ⟺ Designer `xsi:type="dcssch:DataSetFieldNestedDataSet"` (witness СценарииОбменовДанными);
    /// `false` ⇒ обычное поле (`…DataSetField` ⟺ `dcssch:DataSetFieldField`).
    #[serde(default)]
    pub nested: bool,
    /// Путь данных (`dataPath` ⟺ `dcssch:dataPath`).
    pub data_path: String,
    /// Имя поля (`field` ⟺ `dcssch:field`).
    pub field: String,
    /// Выражение представления поля, если задано (`presentationExpression` ⟺
    /// `dcssch:presentationExpression`; witness CommonForm.ДокументыПоДоговору
    /// `НСтр(Дополнительно)`). Простой текст-выражение; идёт за `field`. ОБА формата несут ⇒
    /// X-сравнимо. `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub presentation_expression: Option<String>,
    /// Заголовок, если задан: ЛОКАЛИЗОВАННЫЙ (EDT `<title><localValue><content>` ⟺ Designer
    /// `<dcssch:title xsi:type="v8:LocalStringType">`) — [`PropertyValue::Localized`]; либо
    /// ПРОСТАЯ строка (EDT `<title><value>Текст</value></title>` ⟺ Designer
    /// `<dcssch:title xsi:type="xs:string">Текст`; witness НаборыДополнительныхРеквизитовИСведений)
    /// — [`PropertyValue::Str`].
    #[serde(default)]
    pub title: Option<PropertyValue>,
    /// Тип значения поля (`valueType` ⟺ `dcssch:valueType`; witness ERP AccumulationRegister
    /// ВыручкаИСебестоимостьПродаж.ФормаСписка). `None` ⇒ отсутствует.
    #[serde(default)]
    pub value_type: Option<crate::ir::value::TypeSpec>,
    /// Ограничение использования ПОЛЯ (`useRestriction` ⟺ `dcssch:useRestriction`, ВЕТКА с
    /// presence-bools; witness ВнешниеПользователи). `None` ⇒ отсутствует.
    #[serde(default)]
    pub use_restriction: Option<DcsUseRestriction>,
    /// Ограничение использования РЕКВИЗИТОВ поля (`attributeUseRestriction` ⟺
    /// `dcssch:attributeUseRestriction`; witness ПубличныеИдентификаторыСинхронизируемыхОбъектов).
    #[serde(default)]
    pub attribute_use_restriction: Option<DcsUseRestriction>,
    /// Оформление ПОЛЯ (`appearance` ⟺ `dcssch:appearance`; witness ERP
    /// InformationRegister.СостоянияCDNПлощадокИСМП.ФормаСписка поле `ДатаНачалаНедоступности`
    /// `Формат=ДЛФ=DT`). Тот же items-параметр-механизм, что у [`DcsCalculatedField::appearance`]
    /// (общие ридер/писатель). Пусто ⇒ тег отсутствует. Позиция: ПОСЛЕ valueType.
    #[serde(default)]
    pub appearance: Vec<DcsSettingsParameterValue>,
    /// Доступные значения ПОЛЯ (`availableValues` ⟺ `dcssch:availableValue`, ПОВТОРЯЕМЫЙ; witness
    /// ERP Document.СчетНаОплатуКлиенту.ФормаСозданияСчетовНаОплату поле `Состояние`). Каждый —
    /// значение + опц. локализованное представление. ОБА формата несут ⇒ X-сравнимо. Пусто ⇒ тег
    /// отсутствует. Позиция: ПОСЛЕ appearance. См. [`DcsAvailableValue`].
    #[serde(default)]
    pub available_values: Vec<DcsAvailableValue>,
}

/// Доступное значение DCS-поля (EDT `<availableValues>` ⟺ Designer `<dcssch:availableValue>`):
/// типизированное значение + опц. представление (простое/локализованное). ОБА формата несут ⇒
/// X-сравнимо. Метамодель: `AvailableValue { value: Value, presentation: Presentation }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsAvailableValue {
    /// Значение (EDT `<value xsi:type="core:*Value">` ⟺ Designer `<dcssch:value xsi:type>`) —
    /// тот же тип-диспатч, что у [`DcsParameter::value`] (витнесс — `StringValue`/`xs:string`).
    pub value: DcsParamValue,
    /// Представление значения, если задано (EDT `<presentation><localValue>` ⟺ Designer
    /// `<dcssch:presentation xsi:type="v8:LocalStringType">`) — title-подобное
    /// [`PropertyValue::Localized`] либо простая [`PropertyValue::Str`]. `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub presentation: Option<PropertyValue>,
}

/// Ограничение использования DCS-поля: presence-bools `field`/`condition`/`group`/`order`
/// (EDT `<field>true</field>…` ⟺ Designer `<dcssch:field>true</dcssch:field>…`, в этом
/// порядке; отсутствующий таг = false). Обе стороны несут одинаково ⇒ X-сравнимо.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DcsUseRestriction {
    /// `<field>true</field>` присутствует.
    pub field: bool,
    /// `<condition>true</condition>` присутствует.
    pub condition: bool,
    /// `<group>true</group>` присутствует.
    pub group: bool,
    /// `<order>true</order>` присутствует.
    pub order: bool,
}

/// Вычисляемое поле динамического списка (EDT `<calculatedFields>` ⟺ Designer
/// `<CalculatedField>`; witness НеудаленныеОбъекты.ФормаСписка): dataPath + expression +
/// опц. локализованный title + опц. useRestriction. ОБА формата несут ⇒ X-сравнимо.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsCalculatedField {
    /// Ordered available values, before valueType in both SDK dialects.
    #[serde(default)]
    pub available_values: Vec<DcsAvailableValue>,
    /// Путь данных (`dataPath` ⟺ `dcssch:dataPath`).
    pub data_path: String,
    /// Выражение (`expression` ⟺ `dcssch:expression`), напр. `4`.
    pub expression: String,
    /// Заголовок (как у [`DcsField::title`]), если задан.
    #[serde(default)]
    pub title: Option<PropertyValue>,
    /// Ограничение использования (`useRestriction`), если задано.
    #[serde(default)]
    pub use_restriction: Option<DcsUseRestriction>,
    /// Выражение представления (`presentationExpression` ⟺ `dcssch:presentationExpression`;
    /// witness ERP InformationRegister.ОперацииСПодключаемымОборудованием.ФормаСписка). Простой
    /// текст-выражение; ПОСЛЕ useRestriction, ДО orderExpressions. `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub presentation_expression: Option<String>,
    /// Выражения упорядочивания (`orderExpression` ⟺ `dcssch:orderExpression`, ПОВТОРЯЕМЫЙ; тот же
    /// witness). Каждое — выражение + направление + авто-порядок. ОБА формата несут ⇒ X-сравнимо
    /// (EDT опускает дефолты orderType=Asc/autoOrder=false, Designer эмитит явно в common-ns).
    /// Пусто ⇒ тег отсутствует. Позиция: ПОСЛЕ presentationExpression, ДО appearance.
    /// См. [`DcsOrderExpression`].
    #[serde(default)]
    pub order_expressions: Vec<DcsOrderExpression>,
    /// Оформление вычисляемого поля (`dcssch:appearance` ⟷ EDT items-форма; witness ERP W13
    /// ОтклоненияВСтоимостиТоваров.ФормаСписка). Пусто ⇒ тег отсутствует.
    #[serde(default)]
    pub appearance: Vec<DcsSettingsParameterValue>,
    /// Тип значения (`dcssch:valueType` ⟷ EDT `valueType`), если задан.
    #[serde(default)]
    pub value_type: Option<crate::ir::value::TypeSpec>,
}

/// Выражение упорядочивания DCS-поля (`orderExpression` ⟺ `dcssch:orderExpression`): выражение +
/// направление + авто-порядок. `DataCompositionOrderExpression` метамодели.
///
/// Дельта диалектов (witness ОперацииСПодключаемымОборудованием): EDT эмитит ТОЛЬКО `<expression>`
/// (orderType=Asc / autoOrder=false — дефолты, опущены), Designer эмитит ВСЕ ТРИ явно, причём его
/// дети сидят в ns `…/data-composition-system/common` (ДЕФОЛТНЫЙ xmlns-оверрайд на КАЖДОМ ребёнке).
/// Канон IR несёт значения (не константы) ⇒ EDT-ридер добивает дефолты, EDT-писатель опускает их —
/// оба диалекта дают ИДЕНТИЧНЫЙ IR (X-сравнимо), round-trip байт-точен в обе стороны.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsOrderExpression {
    /// Выражение (`expression`), напр. `Дата`.
    pub expression: String,
    /// Направление (`orderType`), напр. `Asc`/`Desc`. EDT-дефолт (при отсутствии) — `Asc`.
    pub order_type: String,
    /// Авто-порядок (`autoOrder`). EDT-дефолт (при отсутствии) — `false`.
    pub auto_order: bool,
}

/// Параметр схемы набора данных DCS (EDT `<parameters>` ⟺ Designer `<Parameter>`): имя +
/// заголовок + тип + значение + флаги. ОБА формата несут ⇒ X-сравнимо.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsParameter {
    /// Parameter expression, preserved as text, before use/availableAsField.
    #[serde(default)]
    pub expression: Option<String>,
    /// Explicit witnessed Always; absence remains absence.
    #[serde(default)]
    pub usage: Option<DcsParameterUse>,
    /// Имя параметра (`name` ⟺ `dcssch:name`).
    pub name: String,
    /// Локализованный заголовок (EDT `<title><localValue><content>` ⟺ Designer `<dcssch:title>`),
    /// если задан.
    #[serde(default)]
    pub title: Option<PropertyValue>,
    /// Тип значения (`valueType` ⟺ `dcssch:valueType`). `None` = ОТСУТСТВУЕТ (некоторые
    /// DCS-параметры несут лишь name/title/useRestriction — witness ПустыеПользователи Файлы).
    /// Корпус DCS-параметров НЕ несёт present-empty `<valueType/>` (0 вхождений), поэтому на
    /// EDT-стороне `None` однозначно ⇒ absent ⇒ на write не эмитим. Type-codec.
    #[serde(default)]
    pub value_type: Option<crate::ir::TypeSpec>,
    /// Значение по умолчанию (EDT `<values xsi:type="core:*Value"/>` ⟺ Designer `<dcssch:value>`).
    /// `None` = элемент ОТСУТСТВУЕТ (12 DCS-параметров корпуса несут лишь name/title/флаги без
    /// `<values>`); на write не эмитим.
    #[serde(default)]
    pub value: Option<DcsParamValue>,
    /// Additional parameter values in original order, after the first value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_values: Vec<DcsParamValue>,
    /// Schema useRestriction has semantic default false. Preserve None/false/true
    /// source presence for native same-source output; installed SDK independently
    /// materializes false from EDT absence (BSP post-EDT native witnesses).
    #[serde(default, serialize_with = "serialize_parameter_restriction")]
    pub use_restriction: Option<bool>,
    /// Native source-only omission witness. The semantic schema default is false;
    /// native same-source regeneration must still preserve the absent node.
    /// Not canonical data: the host retains lexical source bytes separately.
    #[serde(skip)]
    pub designer_omitted_use_restriction: bool,
    /// Разрешён список значений (`valueListAllowed` ⟺ `dcssch:valueListAllowed`) — presence-bool.
    #[serde(default)]
    pub value_list_allowed: bool,
    /// Доступен как поле (`availableAsField` ⟺ `dcssch:availableAsField`): EDT/Designer эмитят
    /// `false` явно, если задан. `None` ⇒ отсутствует.
    #[serde(default)]
    pub available_as_field: Option<bool>,
}

/// Значение параметра DCS (EDT `<values xsi:type="core:*Value"/>` ⟺ Designer `<dcssch:value>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DcsParamValue {
    /// Design-time symbolic value: native dcscor:DesignTimeValue, EDT nested value/value.
    DesignTimeValue(String),
    /// `core:UndefinedValue` ⟺ Designer `xsi:nil="true"` (пустое значение).
    Undefined,
    /// `core:BooleanValue` (EDT) ⟺ Designer `xsi:type="xs:boolean"` с текстом (`false`/`true`).
    Boolean(String),
    /// `core:StringValue` с `<value>текст</value>`-ребёнком (EDT) ⟺ Designer
    /// `xsi:type="xs:string"` с текстом (пустой ⇒ самозакрытие; witness МашиночитаемыеДоверенности).
    Str(String),
    /// `core:DateValue` с `<value>0001-01-01T00:00:00</value>` (EDT) ⟺ Designer
    /// `xsi:type="xs:dateTime"` с текстом (witness МашиночитаемыеДоверенности).
    Date(String),
    /// `core:UuidValue value="GUID"` (EDT, value-АТРИБУТ) ⟺ Designer `xsi:type="v8:UUID"` с
    /// текстом (witness ВнешниеПользователи/Пользователи).
    Uuid(String),
    /// `core:NumberValue` (EDT) ⟺ Designer `xsi:type="xs:decimal"` с текстом (witness ERP
    /// AccountingRegister Международный.ФормаСписка).
    Decimal(String),
    /// Значение-ТИП: EDT `<values xsi:type="core:TypeValue"><value>Undefined</value></values>` ⟺
    /// Designer `<dcssch:value xmlns:dNpM="…8.2/data/types" xsi:type="v8:Type">dNpM:Undefined`
    /// (witness ERP Catalog.КлючиРеестраДокументов.ФормаВыбора DCS-параметр `ТипЗначенияКлюча`).
    /// Хранится ЛОКАЛЬНОЕ ИМЯ типа в types-ns (`Undefined`) — беспрефиксный канон, общий обоим
    /// диалектам (X-сравнимо); Designer-писатель ре-надевает авто-префикс types-ns по локусу.
    /// Витнесснут ТОЛЬКО примитив types-ns; иной тип — §1.0-отказ в кодеках.
    TypeValue(String),
}

/// DCS-настройки динамического списка (Designer `<ListSettings>` dcsset-блок ⟺ EDT САЙДКАР
/// `Attributes/<Имя>/ExtInfo/ListSettings.dcss` — тот же контент, settings-ns дефолтный вместо
/// `dcsset:`; читает/пишет `formats_xml::form::{read,write}_list_settings_dcss`, привязывает
/// pipeline). Группы (filter/dataParameters/order/conditionalAppearance, КАЖДАЯ опциональна —
/// витнесс `<ListSettings/>` целиком пустой: ВерсииПодсистемОбластейДанных) + опц. корневые
/// структурные `dcsset:item` (ДоступныеАнкеты) + items-мета (каждая опциональна). Типизированы
/// (§1.0 — не Raw): незнакомый под-элемент / xsi:type → ошибка.
///
/// ПУСТЫЕ настройки ([`Self::is_empty`]) ⟺ Designer `<ListSettings/>` ⟺ EDT сайдкар ОТСУТСТВУЕТ
/// (227 сайдкаров на 229 динсписков SSL; ×2 без файла — ровно те, у кого Designer-тег пуст).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DcsListSettings {
    /// ERP-флавор конверта `.dcss`: корень БЕЗ `xmlns:pal` (ценз ERP 3673/3673; SSL несёт
    /// pal). Presence-бит для byte-exact re-emit; `false` = SSL-флавор.
    #[serde(default)]
    pub envelope_without_pal: bool,
    /// Группа отбора (`dcsset:filter`). `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub filter: Option<DcsSettingsGroup>,
    /// Параметры данных (`dcsset:dataParameters` — `dcscor:item xsi:type=
    /// "dcsset:SettingsParameterValue"`-список; witness СценарииОбменовДанными.
    /// НастройкаРасписанияОбменовДанными). Пусто ⇒ тег отсутствует. Позиция: ПОСЛЕ filter,
    /// ДО order.
    #[serde(default)]
    pub data_parameters: Vec<DcsSettingsParameterValue>,
    /// Группа порядка (`dcsset:order`).
    #[serde(default)]
    pub order: Option<DcsSettingsGroup>,
    /// Группа условного оформления (`dcsset:conditionalAppearance`).
    #[serde(default)]
    pub conditional_appearance: Option<DcsSettingsGroup>,
    /// КОРНЕВЫЕ структурные элементы (`dcsset:item xsi:type="dcsset:StructureItemGroup"` —
    /// ПОСЛЕ conditionalAppearance, ДО itemsViewMode; witness ДоступныеАнкеты.АрхивАнкет).
    #[serde(default)]
    pub structure_items: Vec<DcsItem>,
    /// Режим показа элементов (`dcsset:itemsViewMode`), напр. `Normal`. `None` ⇒ отсутствует
    /// (witness УничтожениеПерсональныхДанных).
    #[serde(default)]
    pub items_view_mode: Option<String>,
    /// GUID пользовательской настройки элементов (`dcsset:itemsUserSettingID`). `None` ⇒
    /// отсутствует (witness Взаимодействия.ФормаСписка).
    #[serde(default)]
    pub items_user_setting_id: Option<String>,
    /// Представление пользовательской настройки элементов (`dcsset:itemsUserSettingPresentation`
    /// ⟺ EDT-сайдкар `<itemsUserSettingPresentation>`; witness ERP
    /// Catalog.ПравилаРаспределенияРасходов.ФормаСпискаВручную реквизит `ПоказателиКЗаполнению`:
    /// `v8:LocalStringType` «Сгруппировать по типу»). Позиция: ПОСЛЕ itemsUserSettingID (последний
    /// ребёнок). `None` ⇒ тег отсутствует. В cf — ячейка `GroupSelectedSettingPresentation`.
    #[serde(default)]
    pub items_user_setting_presentation: Option<DcsPresentation>,
}

impl DcsListSettings {
    /// Настроек НЕТ ни одной (все регионы пусты) — сериализуется как Designer `<ListSettings/>`
    /// и НЕ порождает EDT-сайдкара `.dcss` (платформа файл не создаёт).
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Настройки — ПЛАТФОРМЕННЫЙ ДЕФОЛТ: три пустые группы (filter/order/conditionalAppearance),
    /// каждая `viewMode=Normal` + КАНОНИЧЕСКИЙ `userSettingID`, плюс `itemsViewMode=Normal` +
    /// канонический `itemsUserSettingID`; без элементов и без параметров данных.
    ///
    /// Это ТОЧНО тот 1155-байтовый сайдкар, который несут 171 из 227 SSL-динсписков (файлы
    /// байт-идентичны). Значение предиката — в `.cf`: ДЕФОЛТ ⟺ бэг динсписка НЕ несёт ни одной
    /// секции настроек (`Order`/`Filter`/`Appearance`/`Group`); ЛЮБОЕ отклонение — как БОГАЧЕ
    /// (56 сайдкаров), так и БЕДНЕЕ (2 формы вообще БЕЗ сайдкара, `<ListSettings/>`) — секции
    /// порождает. Проверено на всех 229 динсписках SSL (`probe_dl_bag check`, 0 расхождений).
    pub fn is_platform_default(&self) -> bool {
        /// Канонический `userSettingID` группы отбора.
        const FILTER_ID: &str = "dfcece9d-5077-440b-b6b3-45a5cb4538eb";
        /// Канонический `userSettingID` группы порядка.
        const ORDER_ID: &str = "88619765-ccb3-46c6-ac52-38e9c992ebd4";
        /// Канонический `userSettingID` группы условного оформления.
        const APPEARANCE_ID: &str = "b75fecce-942b-4aed-abc9-e6a02e460fb3";
        /// Канонический `itemsUserSettingID` структуры.
        const ITEMS_ID: &str = "911b6018-f537-43e8-a417-da56b22f9aec";

        fn default_group(g: &Option<DcsSettingsGroup>, id: &str) -> bool {
            g.as_ref().is_some_and(|g| {
                g.items.is_empty()
                    && g.view_mode.as_deref() == Some("Normal")
                    && g.user_setting_id.as_deref() == Some(id)
            })
        }
        default_group(&self.filter, FILTER_ID)
            && default_group(&self.order, ORDER_ID)
            && default_group(&self.conditional_appearance, APPEARANCE_ID)
            && self.data_parameters.is_empty()
            && self.structure_items.is_empty()
            && self.items_view_mode.as_deref() == Some("Normal")
            && self.items_user_setting_id.as_deref() == Some(ITEMS_ID)
            && self.items_user_setting_presentation.is_none()
    }
}

/// Группа DCS-настроек (`dcsset:filter`/`order`/`conditionalAppearance`): элементы + опц.
/// viewMode + опц. userSettingID (в этом порядке; элементы ДО меты; каждая мета независимо
/// опциональна — витнессы Взаимодействия/УничтожениеПерсональныхДанных).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DcsSettingsGroup {
    /// Элементы группы (`dcsset:item`), в исходном порядке. Пусто ⇒ группа-заглушка (лишь мета).
    pub items: Vec<DcsItem>,
    /// Режим показа (`dcsset:viewMode`), напр. `Normal`. `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub view_mode: Option<String>,
    /// GUID пользовательской настройки (`dcsset:userSettingID`). `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub user_setting_id: Option<String>,
    /// ГРУППОВОЕ представление пользовательской настройки (`dcsset:userSettingPresentation`) —
    /// ПОСЛЕ `userSettingID`. Витнесс ERP Catalog.ОтветственныеЗаАктуализациюТокенов…ФормаСписка:
    /// `<filter>`-группа несёт `<userSettingPresentation xsi:type="xs:string">Ответственный`.
    /// `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub user_setting_presentation: Option<DcsPresentation>,
}

/// Параметр DCS-настроек (`dcscor:item xsi:type="dcsset:SettingsParameterValue"` — внутри
/// `dcsset:dataParameters` либо `dcsset:appearance`): опц. use + parameter + value + опц.
/// userSettingID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsSettingsParameterValue {
    /// Использование (`dcscor:use`, явный bool). `None` ⇒ тег отсутствует (appearance-витнессы).
    #[serde(default)]
    pub used: Option<bool>,
    /// Имя параметра (`dcscor:parameter`), напр. `ЦветТекста`.
    pub parameter: String,
    /// Значение (`dcscor:value`), типизированное по xsi:type/nil. `None` ⇒ тег ОТСУТСТВУЕТ —
    /// неиспользуемый dataParameter (`<dcscor:use>false</dcscor:use>` + `<dcscor:parameter>` без
    /// value; witness ERP ×7 designer-блоков, напр. ЗагрузкаДанныхИзФайла/документные формы).
    #[serde(default)]
    pub value: Option<DcsCorValue>,
    /// GUID пользовательской настройки (`dcsset:userSettingID`). `None` ⇒ отсутствует.
    #[serde(default)]
    pub user_setting_id: Option<String>,
}

/// Значение `dcscor:value` параметра DCS-настроек, по `xsi:type`/`xsi:nil`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DcsCorValue {
    /// Symbolic design-time parameter value, also present in EDT .dcss sidecars.
    DesignTimeValue(String),
    /// `xsi:nil="true"` (пустое значение; witness dataParameters УзелИнформационнойБазы).
    Nil,
    /// `xsi:type="v8ui:Color"` — текст (напр. `style:ТекстЗапрещеннойЯчейкиЦвет`).
    Color(String),
    /// `xsi:type="v8:LocalStringType"` — `<v8:item><v8:lang>/<v8:content>`-пары.
    LocalString(Vec<(Lang, String)>),
    /// `xsi:type="xs:boolean"` — текст `false`/`true` (witness ERP CCT
    /// АналитикиСтатейБюджетов.НастройкаТипаАналитики).
    Boolean(String),
    /// `xsi:type="v8ui:Font"` — атрибутная font-форма (witness ERP FilterCriterion
    /// ЗадачиПоЭкземпляруБюджета.ФормаСписка).
    Font(FontRef),
    /// `xsi:type="xs:string"` ⟷ EDT `core:StringValue` (witness ERP W13 appearance
    /// CalculatedField ОтклоненияВСтоимостиТоваров.ФормаСписка).
    Str(String),
    /// `xsi:type="dcscor:Field"` — текст (имя поля; witness ERP appearance
    /// МероприятияТрудовойДеятельности.ФормаВыбораСобытий `Текст`=`ОписаниеДолжности`).
    Field(String),
    /// `xsi:type="v8ui:HorizontalAlign"` — текст (напр. `Justify`; witness ERP appearance
    /// ПечатьЭтикетокИЦенников.ВыборВариантаЗаполненияУпаковками `ГоризонтальноеПоложение`).
    HorizontalAlign(String),
    /// `xsi:type="xs:decimal"` — текст (напр. `0`; witness ERP dataParameter
    /// СреднийЗаработокСЭДО.ФормаСписка `Год`).
    Decimal(String),
}

/// Представление DCS-элемента (`dcsset:presentation` / `dcsset:userSettingPresentation`),
/// диспетчеризованное по `xsi:type`. Витнессы ERP: FilterItemComparison/FilterItemGroup/
/// CA-item несут локализованное имя отбора/оформления (`xs:string` — 163+ вхождений,
/// `v8:LocalStringType` — 44).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DcsPresentation {
    /// `xsi:type="xs:string"` — текст (пустой ⇒ самозакрытие `<…presentation xsi:type="xs:string"/>`).
    Str(String),
    /// `xsi:type="v8:LocalStringType"` — `<v8:item><v8:lang>/<v8:content>`-пары.
    LocalString(Vec<(Lang, String)>),
}

/// Поле выбора условного оформления (`dcsset:selection` → `dcsset:item`): опц. `use` + `field`.
/// Почти всегда только `field`; ровно 1 witness ERP несёт ещё `<dcsset:use>` (структурен ⇒
/// моделируем, а не роняем).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DcsSelectionField {
    /// Использование (`dcsset:use`, явный bool). `None` ⇒ тег отсутствует.
    #[serde(default)]
    pub used: Option<bool>,
    /// Оформляемое поле (`dcsset:field`).
    pub field: String,
}

/// Элемент DCS-группы (`dcsset:item`), диспетчеризованный по `xsi:type` (элемент УСЛОВНОГО
/// ОФОРМЛЕНИЯ — БЕЗ xsi:type).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DcsItem {
    /// `dcsset:FilterItemComparison`: use? / left(поле) / comparisonType / right? / viewMode? /
    /// userSettingID? (все опц. кроме left+comparisonType; витнессы Взаимодействия/
    /// ПубличныеИдентификаторы/РаботаСФайлами).
    FilterComparison {
        /// Использовать (`dcsset:use`, явный bool). `None` ⇒ тег отсутствует.
        used: Option<bool>,
        /// Левый операнд (`dcsset:left`), текст. Тип — в `left_type` (обычно `dcscor:Field`).
        left_field: String,
        /// `xsi:type` левого операнда (`dcscor:Field` — 3145/3147; `xs:boolean` — 2 witness ERP
        /// ИндексацияШтатногоРасписания.ФормаДокумента). Сохраняется для byte-exact re-emit.
        #[serde(default)]
        left_type: String,
        /// Вид сравнения (`dcsset:comparisonType`), напр. `Equal`/`Greater`.
        comparison_type: String,
        /// Правые значения (`dcsset:right`, ПОВТОРЯЕМЫЙ при InList — witness ERP
        /// СогласованиеЗакупки.ФормаСписка: два right'а). Пусто ⇒ тег отсутствует (witness
        /// Взаимодействия «Предмет»).
        right: Vec<DcsRightValue>,
        /// Представление отбора (`dcsset:presentation`; witness ERP СтавкиНДСНоменклатуры).
        /// `None` ⇒ тег отсутствует. Позиция: ПОСЛЕ right, ДО viewMode.
        #[serde(default)]
        presentation: Option<DcsPresentation>,
        /// Режим показа (`dcsset:viewMode`). `None` ⇒ отсутствует.
        view_mode: Option<String>,
        /// GUID пользовательской настройки (`dcsset:userSettingID`). `None` ⇒ отсутствует.
        user_setting_id: Option<String>,
        /// Представление пользовательской настройки (`dcsset:userSettingPresentation`; witness ERP
        /// СтавкиНДСНоменклатуры). `None` ⇒ отсутствует. Позиция: ПОСЛЕ userSettingID (последний).
        #[serde(default)]
        user_setting_presentation: Option<DcsPresentation>,
    },
    /// `dcsset:OrderItemField`: use? / field / orderType / viewMode? (витнессы Задание/
    /// СертификатыКлючей…/ВерсииФайлов).
    OrderField {
        /// Использовать (`dcsset:use`, явный bool). `None` ⇒ тег отсутствует.
        used: Option<bool>,
        /// Поле упорядочивания (`dcsset:field`), текст.
        field: String,
        /// Направление (`dcsset:orderType`), напр. `Asc`/`Desc`.
        order_type: String,
        /// Режим показа (`dcsset:viewMode`). `None` ⇒ отсутствует.
        view_mode: Option<String>,
    },
    /// Элемент УСЛОВНОГО ОФОРМЛЕНИЯ (`dcsset:item` БЕЗ xsi:type, внутри
    /// `dcsset:conditionalAppearance` / форм-уровневого `<ConditionalAppearance>`): selection +
    /// filter-элементы + appearance (витнессы МашиночитаемыеДоверенности/РаботаСФайлами).
    ConditionalAppearance {
        /// Использовать (`dcsset:use`, явный bool; witness ERP — 41 CA-элемента). `None` ⇒ тег
        /// отсутствует. Позиция: ПЕРЕД selection.
        #[serde(default)]
        used: Option<bool>,
        /// Оформляемые поля (`dcsset:selection`): `None` ⇒ САМОЗАКРЫТЫЙ `<dcsset:selection/>`
        /// (все поля); `Some(поля)` ⇒ вложенные `<dcsset:item>[<dcsset:use>]<dcsset:field>Поле`.
        selection: Option<Vec<DcsSelectionField>>,
        /// Элементы отбора (`dcsset:filter` → `dcsset:item`-ы), рекурсивно.
        filter: Vec<DcsItem>,
        /// Оформление (`dcsset:appearance` → `dcscor:item xsi:type="dcsset:SettingsParameterValue"`).
        appearance: Vec<DcsSettingsParameterValue>,
        /// Представление (`dcsset:presentation`; witness ERP — 209 CA-элементов несут имя).
        /// `None` ⇒ тег отсутствует. Позиция: ПОСЛЕ appearance.
        #[serde(default)]
        presentation: Option<DcsPresentation>,
        /// Режим показа (`dcsset:viewMode`). `None` ⇒ отсутствует. Позиция: ПОСЛЕ presentation.
        #[serde(default)]
        view_mode: Option<String>,
        /// GUID пользовательской настройки (`dcsset:userSettingID`). `None` ⇒ отсутствует.
        /// Позиция: ПОСЛЕ viewMode (последний).
        #[serde(default)]
        user_setting_id: Option<String>,
    },
    /// `dcsset:StructureItemGroup`: `dcsset:groupItems` (поля группировки) + опц. ВЛОЖЕННЫЕ
    /// структурные `dcsset:item` ПОСЛЕ `groupItems` (дерево под-группировок; witness
    /// ДоступныеАнкеты.АрхивАнкет — 0 вложений; ERP ВидыЦен.ФормаНастройки… — глубина 5).
    ///
    /// Designer/edt-сайдкар держат структуру ВЛОЖЕННОЙ (рекурсивный `StructureItemGroup`); cf-бэг
    /// `<GroupItems>`-секции держит её ПЛОСКОЙ — все `GroupItemField` дерева в pre-order-обходе
    /// (сверено с erp.cf ВидыЦен: `СсылкаВидЦен, Валюта, Выбран, Картинка, СпособЗаданияЦены,
    /// Статус`). Уплощение — [`super`-writer] cf-секции.
    StructureGroup {
        /// Поля группировки этого уровня (`dcsset:groupItems` → `GroupItemField`-ы).
        group_items: Vec<DcsItem>,
        /// Вложенные под-структуры (`dcsset:item xsi:type="StructureItemGroup"` ПОСЛЕ
        /// `groupItems`), рекурсивно. Пусто ⇒ лист дерева.
        #[serde(default)]
        nested: Vec<DcsItem>,
    },
    /// `dcsset:FilterItemGroup`: композит отбора/условного оформления (And/Or/Not-группа).
    /// Несёт `groupType` + вложенные `dcsset:item`-ы (FilterItemComparison/FilterItemGroup),
    /// опц. `use`/`presentation`/`viewMode` (witness ERP W19 ВидыБюджетов.ФормаЭлемента).
    FilterGroup {
        /// Использовать (`dcsset:use`, явный bool). `None` ⇒ тег отсутствует. Позиция: ПЕРВЫЙ.
        used: Option<bool>,
        /// Тип группы (`dcsset:groupType`), напр. `OrGroup`/`AndGroup`/`NotGroup`.
        group_type: String,
        /// Вложенные элементы отбора (`dcsset:item`-ы), рекурсивно. Пусто ⇒ нет вложений.
        items: Vec<DcsItem>,
        /// Представление (`dcsset:presentation`). `None` ⇒ отсутствует. Позиция: ПОСЛЕ items.
        presentation: Option<DcsPresentation>,
        /// Режим показа (`dcsset:viewMode`). `None` ⇒ отсутствует. Позиция: ПОСЛЕ presentation.
        view_mode: Option<String>,
        /// Ordered group identity, present in UH native and EDT sidecar.
        #[serde(default)]
        user_setting_id: Option<String>,
    },
    /// `dcsset:OrderItemAuto`: авто-упорядочивание — ПУСТОЙ самозакрытый
    /// `<dcsset:item xsi:type="dcsset:OrderItemAuto"/>` (witness ERP — 23/23 без детей).
    OrderAuto,
    /// `dcsset:GroupItemField`: use? / field / groupType / periodAdditionType /
    /// periodAdditionBegin / periodAdditionEnd (witness ДоступныеАнкеты.АрхивАнкет).
    GroupField {
        /// Использовать (`dcsset:use`, явный bool). `None` ⇒ тег отсутствует.
        used: Option<bool>,
        /// Поле группировки (`dcsset:field`).
        field: String,
        /// Тип группировки (`dcsset:groupType`), напр. `Items`.
        group_type: String,
        /// Тип дополнения периода (`dcsset:periodAdditionType`), напр. `None`.
        period_addition_type: String,
        /// Начало дополнения периода (`dcsset:periodAdditionBegin xsi:type="xs:dateTime"`).
        period_addition_begin: String,
        /// Конец дополнения периода (`dcsset:periodAdditionEnd xsi:type="xs:dateTime"`).
        period_addition_end: String,
    },
}

/// Правое значение сравнения фильтра (`dcsset:right`), диспетчеризованное по `xsi:type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DcsRightValue {
    /// Explicit undefined operand: Designer/EDT `right xsi:nil="true"`.
    Undefined,
    /// `xs:boolean`: текст `true`/`false`.
    Boolean(String),
    /// `xs:decimal`: текст (напр. `0`; witness Взаимодействия).
    Decimal(String),
    /// `xs:string`: текст (пустой ⇒ самозакрытие `<dcsset:right xsi:type="xs:string"/>`;
    /// witness ПубличныеИдентификаторыСинхронизируемыхОбъектов).
    Str(String),
    /// `v8:StandardBeginningDate`: вложенный `<v8:variant xsi:type=
    /// "v8:StandardBeginningDateVariant">вариант</v8:variant>` (напр. `BeginningOfThisDay`) +
    /// опц. `<v8:date>0001-01-01T00:00:00</v8:date>` (вариант `Custom`; witness Взаимодействия).
    StandardBeginningDate {
        /// Вариант стандартной даты.
        variant: String,
        /// Дата варианта `Custom`, если несёт.
        date: Option<String>,
    },
    /// `dcscor:Field`: текст — имя поля (witness БлокировкаРаботыПользователей).
    Field(String),
    /// `dcscor:DesignTimeValue`: текст — ссылка времени конфигурирования (напр.
    /// `Перечисление.ТехническиеСтатусыМЧД.Отменена`; witness МашиночитаемыеДоверенности).
    DesignTimeValue(String),
    /// `v8:Type` с ИНЛАЙН `xmlns:d8p1="http://v8.1c.ru/8.2/data/types"`: текст — QName типа
    /// (витнессированно РОВНО `d8p1:Undefined`; Взаимодействия). Инлайн-ns фиксирован.
    TypeQName(String),
    /// `v8:ValueListType` — ПУСТОЙ список значений: `<v8:valueType/>` (самозакрытый) +
    /// `<v8:lastId xsi:type="xs:decimal">last_id</v8:lastId>` (witness ERP FilterItemComparison
    /// InList ДиспетчированиеПроизводства.`Выполнение`; ценз 9/9 — все пусты, lastId=`-1`).
    /// Непустой список (с `<v8:item>`-элементами) не витнесснут ⇒ ридер отказывает (§1.0).
    ValueList {
        /// Значение `<v8:lastId xsi:type="xs:decimal">` (напр. `-1`).
        last_id: String,
    },
}

/// Explicit schema parameter usage witnessed in independent UH native/EDT pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DcsParameterUse {
    /// Always apply the parameter; unspecified usage is stored as None.
    Always,
}

/// Only the witnessed DCS schema-parameter default: absence has false semantics.
/// Presence remains in the private fields for exact native same-source output.
fn serialize_parameter_restriction<S: serde::Serializer>(
    value: &Option<bool>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_bool(value.unwrap_or(false))
}
