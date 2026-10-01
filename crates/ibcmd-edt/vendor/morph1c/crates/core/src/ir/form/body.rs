use super::*;

/// КОРЕНЬ под-IR формы (`<form:Form>` EDT / `<Form>` Designer).
///
/// Несёт:
/// * `attributes` — ФОРМ-УРОВНЕВЫЕ свойства (`AutoTitle`/`CommandBarLocation`/`Group`/…)
///   по каноническому [`FieldId`], в порядке форм-спека (дефолты не хранятся, §1.1);
/// * `events` — форм-уровневые обработчики (`OnCreateAtServer`, `OnOpen`, …);
/// * `items` — КОРНЕВОЙ список контролов (рекурсивное дерево [`FormItem`]).
///
/// Форм-уровневые ДАННЫЕ-атрибуты (`<attributes>`/`<parameters>` EDT, `<Attributes>`/
/// `<Parameters>` Designer) и фиксированные служебные дети (`autoCommandBar`,
/// `commandInterface`) — отдельные регионы; их моделирование наполняется по мере роста
/// каталога (foundation несёт скелет `items`+`attributes`+`events`). Тело модуля формы
/// (`Module.bsl`) — ВНЕ под-IR (отдельный артефакт, как прочие модули).
/// Один пункт формоуровневого командного интерфейса (`<cmiFragmentRecord>` EDT / `<Item>`
/// Designer). Cross-omission witness — 101⟷101 пунктов SSL (CommonForms + object-формы,
/// s15-descent):
///
/// * `type` — EDT опускает `Auto` (98/101), эмитит `Added` (3); Designer `<Type>` ВСЕГДА.
/// * `group` ⟺ `CommandGroup` — опциональны оба (23⟷23, значения зеркальны).
/// * `index` — ПОЛЬЗОВАТЕЛЬСКИЙ порядок размещения (НЕ дериват: witness Документы/…/ФормаСписка
///   несёт index 2,3,1,0 при ordinal 0,1,2,3). НЕЗАВИСИМ от `group`: EDT эмитит `<index>` ВСЕГДА
///   при `group` (включая 0), И как index-БЕЗ-группы (размещение по индексу на КОРНЕ панели;
///   census ERP 325, значения ≥1); Designer `<Index>` опускает `0` (под группой = index 0).
///   `group`-БЕЗ-index EDT НЕ эмитит (0/8194) ⇒ §1.0-отказ ридера.
/// * видимость — ТРИ-состояние: не задана (EDT нет `<userVisible>`; Designer нет
///   DefaultVisible/Visible — 1⟷1) / `false` (EDT пустой `<userVisible/>`; Designer
///   `DefaultVisible=false` + `<Visible><xr:Common>false` — 97⟷97) / `true` (EDT
///   `<common>true`; Designer `DefaultVisible=false` БЕЗ `<Visible>` — 3⟷3).
///   `DefaultVisible` эмитится Designer'ом ⟺ видимость ЗАДАНА (всегда литерал `false`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormCiItem {
    /// Полное имя команды (`CommonCommand.ИсторияИзменений`, `<Kind>.<Name>.StandardCommand.<Cmd>`, …).
    pub command: String,
    /// Тип пункта (`Auto`/`Added`): EDT `<type>` опускает `Auto`; Designer `<Type>` эмитит всегда.
    pub ty: String,
    /// Typed command DataPath parameter (EDT commandParameter / XML Attribute).
    #[serde(default)]
    pub command_parameter: Option<String>,
    /// Группа команд (`FormCommandBarImportant`/`CommandGroup.<Имя>`): EDT `<group>` ⟺
    /// Designer `<CommandGroup>`.
    pub group: Option<String>,
    /// Порядок размещения (`index`/`<Index>`): `Some(N)` при group+index (N≥0; Designer опускает
    /// 0) ЛИБО при index-БЕЗ-группы (размещение по индексу на КОРНЕ панели, N≥1; census ERP 325).
    /// `None` ⇒ ни группы, ни индекса. `group`-БЕЗ-index не витнесснут ⇒ §1.0-отказ ридера.
    pub index: Option<i64>,
    /// Общая видимость (три-состояние; см. док типа). При РОЛЕВЫХ правах несёт ОБЩИЙ флаг
    /// (`<common>` ⟺ `<xr:Common>`), а пер-ролевые исключения — в [`Self::user_visible_roles`].
    pub user_visible: Option<bool>,
    /// РОЛЕВЫЕ права видимости пункта КИ: `(роль, значение)` в исходном порядке. Пусто =
    /// безролевой пункт (обычный случай, 100/101). Witness ЗаказКлиента.ФормаСписка/…
    /// ФормаСпискаДокументов: EDT `<userVisible><common>true</common><for><value>B</value>
    /// <role>R</role></for>×N` ⟺ Designer `<Visible><xr:Common>B</xr:Common>
    /// <xr:Value name="R">B</xr:Value>×N</Visible>`. cf несёт лишь общий флаг (роли — отдельный
    /// поток прав, зеркало Command/Use). X-сравнимо (оба формата несут один список).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_visible_roles: Vec<(String, bool)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormBody {
    /// Native XML 2.20 lexical presence of Auto CheckBoxType. This source-only
    /// spelling facet does not change the independently typed CheckBoxType value.
    /// It is excluded from semantic serialization; edited nondefault values win.
    #[serde(skip)]
    pub designer_checkbox_auto_presence: std::collections::BTreeMap<i64, bool>,
    /// Typed referenced CommonPicture defaults, populated by whole-config binding.
    /// Projection context only; independent per-use BOOL/pixel remain semantic IR.
    #[serde(skip)]
    pub common_picture_transparency: std::collections::BTreeMap<String, bool>,
    /// Native source spelling must remain exact; derived SDK path glyphs are EDT-only.
    #[serde(skip)]
    pub designer_path_spelling: bool,
    /// Локализованный заголовок формы (`<title>`/`<Title>`), если задан. Оба формата
    /// эмитят его явно (X-сравнимо). `None` ⇒ форма без заголовка (как пилот).
    pub title: Option<PropertyValue>,
    /// Форм-уровневые свойства по каноническому `FieldId`, в порядке форм-спека.
    pub attributes: Vec<(FieldId, PropertyValue)>,
    /// Форм-уровневые обработчики событий в исходном порядке.
    pub events: Vec<FormEvent>,
    /// Корневой список контролов (рекурсивное дерево) в исходном порядке.
    pub items: Vec<FormItem>,
    /// Автоматическая командная панель формы (`autoCommandBar`/`AutoCommandBar`), если
    /// присутствует. Фиксированный служебный контрол (`id == -1`).
    pub auto_command_bar: Option<AutoCommandBar>,
    /// Несёт ли форма фиксированный блок `<commandInterface>`/`<CommandInterface>`
    /// (EDT: `<navigationPanel/><commandBar/>`; Designer — отдельная конвенция). Presence-
    /// маркер: `true` ⇒ форма несёт блок (EDT всегда для управляемой формы; Designer его
    /// не сериализует — синтезируется из presence AutoCommandBar). ПУСТОЙ блок кодируется
    /// только этим bool; НЕПУСТОЕ содержимое — в [`Self::form_ci_navigation_panel`]/
    /// [`Self::form_ci_command_bar`] (тогда EDT эмитит наполненный блок, Designer — явный
    /// `<CommandInterface>`).
    pub command_interface: bool,
    /// НАПОЛНЕННЫЙ регион `navigationPanel` формоуровневого командного интерфейса (EDT
    /// `<commandInterface><navigationPanel><cmiFragmentRecord>…`; Designer `<CommandInterface>
    /// <NavigationPanel><Item>…`). Пусто ⇒ панель пуста (обычный случай). Requires
    /// `command_interface == true`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_ci_navigation_panel: Vec<FormCiItem>,
    /// НАПОЛНЕННЫЙ регион `commandBar` формоуровневого командного интерфейса (EDT
    /// `<commandInterface><commandBar><cmiFragmentRecord>…`; Designer `<CommandInterface>
    /// <CommandBar><Item>…`). Пусто ⇒ панель пуста (обычный случай). Requires
    /// `command_interface == true`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_ci_command_bar: Vec<FormCiItem>,
    /// Заголовок группы кнопок создания (`createButtonsGroupTitle` ⟺
    /// `CreateButtonsGroupTitle`) — локализованная строка у форм списка с переопределённой
    /// кнопкой «Создать» (witness ПроизводственныеКалендари/Файлы/НаборыДопРеквизитов/
    /// РаботаСФайлами.ПрисоединенныеФайлы). EDT — после `showCloseButton`; Designer — перед
    /// `ShowCommandBar`/`AutoCommandBar`. ОБА формата несут ⇒ X-сравним.
    #[serde(default)]
    pub create_buttons_group_title: Option<PropertyValue>,
    /// ДАННЫЕ-реквизиты формы (`<attributes>`/`<Attributes>`) в исходном порядке.
    pub data_attributes: Vec<FormDataAttribute>,
    /// ФОРМ-уровневое условное оформление — DESIGNER-ONLY блок `<ConditionalAppearance>` с
    /// `dcsset:item`-элементами ПОСЛЕ всех `<Attribute>` ВНУТРИ `<Attributes>` (witness
    /// РаботаСФайлами.ВерсияПрисоединенногоФайла). EDT держит его в САЙДКАРЕ
    /// `ConditionalAppearance.dcssca` (НЕ в `Form.form`) ⇒ нормализуется ДО X (как
    /// [`DynamicListAttrExt::list_settings`]). Пусто ⇒ блок отсутствует.
    #[serde(default)]
    pub conditional_appearance: Vec<DcsItem>,
    /// ERP-флавор конверта `.dcssca`: корень БЕЗ `xmlns:lf` И `xmlns:pal` (ценз ERP
    /// 506/506; SSL несёт полный 13-ns блок). Пара опциональна АТОМАРНО (половинчатый
    /// флавор — §1.0-отказ ридера). `false` = SSL-флавор.
    #[serde(default)]
    pub ca_envelope_without_lf_pal: bool,
    /// Параметры формы (`<parameters>`/`<Parameters>`) в исходном порядке.
    pub parameters: Vec<FormParameter>,
    /// Пользовательские команды формы (`<formCommands>` EDT / `<Commands><Command>`
    /// Designer) в исходном порядке. X-сравнимы (оба формата несут одинаково).
    pub commands: Vec<FormCommand>,
    /// ФОРМ-УРОВНЕВЫЕ исключённые стандартные команды (`<excludedCommands>`-листы EDT /
    /// `<CommandSet><ExcludedCommand>` Designer) в исходном порядке (оба формата несут
    /// ОДИН список в одном порядке — X-сравним). Пусто ⇒ регион отсутствует.
    pub excluded_commands: Vec<String>,
    /// КОРНЕВОЙ extInfo формы (`<extInfo xsi:type="form:*FormExtInfo">` — прямой ребёнок
    /// `<form:Form>`), если несёт. Тип формы (`Object`/`DynamicList`/`Constants`/…) — EDT-ONLY
    /// (Designer НЕ несёт маркера типа: он неявен), поэтому [`FormRootExtInfo::kind`]
    /// нормализуется ДО X. СОБСТВЕННЫЕ обработчики extInfo (EDT `<extInfo><handlers>` —
    /// `AfterWrite`/`OnWriteAtServer`/…) в Designer слиты в единый КОРНЕВОЙ `<Events>` вместе
    /// с обычными форм-событиями (платформенный порядок); ⇒ X сравнивает форм-события КАК
    /// МНОЖЕСТВО (объединив [`Self::events`] и extInfo-события).
    #[serde(default)]
    pub root_ext_info: Option<FormRootExtInfo>,
    /// Форм-уровневая информация формы отчёта (`form:ReportFormExtInfo` EDT ⟺ корневые
    /// `ReportFormType`/… Designer). ОБА формата несут её содержимое ⇒ X-сравнима НАПРЯМУЮ
    /// (не нормализуется). `None` ⇒ форма не является формой отчёта.
    #[serde(default)]
    pub report_form: Option<ReportFormInfo>,
    /// Состав командной панели мобильного устройства (`<mobileDeviceCommandBarContent>` EDT ⟺
    /// `<MobileDeviceCommandBarContent>` Designer) — КОРНЕВОЙ список скалярных значений (имён
    /// команд). EDT: повторяемый `<value xsi:type="core:StringValue"><value>X</value></value>`;
    /// Designer: `<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState>
    /// <xr:Value xsi:type="xs:string">X</xr:Value></xr:Item>`. Каждый элемент — скалярное значение
    /// ([`PropertyValue::Value`], через общий value-codec) ⇒ X-сравним НАПРЯМУЮ (не нормализуется).
    /// Пусто ⇒ регион отсутствует (совместимо со всем прежним корпусом).
    #[serde(default)]
    pub mobile_device_command_bar: Vec<PropertyValue>,
    /// ФОРМ-уровневое использование для групп/элементов (`useForFoldersAndItems` ⟺
    /// `UseForFoldersAndItems`) — только у иерархических форм (Catalog/ChartOfCharacteristicTypes:
    /// ФормаЭлемента `Items`, ФормаГруппы `Folders`). Кодировка РАСХОДИТСЯ: EDT держит тег ВНУТРИ
    /// корневого `<extInfo xsi:type="form:*FormExtInfo">` и ОПУСКАЕТ дефолт `Items` (эмитит лишь
    /// `Folders`); Designer эмитит ПРЯМЫМ ребёнком корня `<Form>` ВСЕГДА (`Items` или `Folders`),
    /// перед `<AutoCommandBar>`. `None` ⇒ неиерархическая форма (тег отсутствует у обоих). Дефолт
    /// `Items` нормализуется ДО X (EDT его не несёт).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_for_folders_and_items: Option<String>,
    /// ФОРМ-уровневые свойства формы объекта-документа (`autoTime`/`usePostingMode`/
    /// `repostOnWrite`) — только у форм объекта Document. Кодировка РАСХОДИТСЯ (как у
    /// report-form/useForFoldersAndItems): EDT держит поля ВНУТРИ корневого `<extInfo
    /// xsi:type="form:DocumentFormExtInfo">` и ОПУСКАЕТ дефолты (`autoTime`=`CurrentOrLast`/
    /// `usePostingMode`=`Auto` не эмитятся никогда; `repostOnWrite` эмитит лишь `true`);
    /// Designer эмитит ВСЕ ТРИ прямыми детьми корня `<Form>` ВСЕГДА (после `<CommandSet>`,
    /// перед `<AutoCommandBar>`). `None` ⇒ форма не является формой объекта-документа.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_form: Option<DocumentFormInfo>,
}

/// Форм-уровневые свойства формы объекта-документа (`form:DocumentFormExtInfo` EDT ⟺ корневые
/// `AutoTime`/`UsePostingMode`/`RepostOnWrite` Designer). Канон X-сравним (EDT-опускаемые
/// дефолты восстанавливаются на чтении).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentFormInfo {
    /// `autoTime`/`AutoTime` — режим автоматической установки времени (EDT-дефолт
    /// `CurrentOrLast`, опускается; Designer эмитит всегда).
    pub auto_time: String,
    /// `usePostingMode`/`UsePostingMode` — режим проведения (EDT-дефолт `Auto`, опускается;
    /// Designer эмитит всегда).
    pub use_posting_mode: String,
    /// `repostOnWrite`/`RepostOnWrite` — перепроведение при записи (EDT эмитит лишь `true`,
    /// опускает `false`; Designer эмитит всегда).
    pub repost_on_write: bool,
}

/// Канонический дефолт `autoTime` (EDT его ОПУСКАЕТ; Designer эмитит всегда). Нормализуется ДО X.
pub const DOCUMENT_AUTO_TIME_DEFAULT: &str = "CurrentOrLast";

/// Канонический дефолт `usePostingMode` (EDT его ОПУСКАЕТ; Designer эмитит всегда).
pub const DOCUMENT_USE_POSTING_MODE_DEFAULT: &str = "Auto";

/// Каноническое дефолтное значение `useForFoldersAndItems` — `Items` (форма элемента). EDT его
/// ОПУСКАЕТ (несёт лишь `Folders`); Designer эмитит всегда. Нормализуется ДО X.
pub const FOLDERS_AND_ITEMS_DEFAULT: &str = "Items";

/// Именованное ТЕЛО формы, прикреплённое к объекту метаданных (спутник объекта, не спек-
/// свойство) — под-IR всей [`FormBody`] плюс необязательный модуль формы.
///
/// Носитель для whole-config-конвейера (`pipeline::form_read`): `MetadataObject.forms`
/// (`Vec<FormItem>`) несёт лишь дерево контролов и НЕ годится как носитель всей формы
/// (attributes/events/data_attributes/commands/…). Для CommonForm форма ОДНА (объект = форма),
/// но `Vec` заложен на случай видов с несколькими подчинёнными формами (Catalog/Document/…).
///
/// * `name` — имя формы (для CommonForm == имя объекта).
/// * `body` — каноническое тело формы, прочитанное byte-exact-кодеком
///   `formats_xml::form::read_form` (формат-нейтральный под-IR §1.6: edt-тело и designer-тело
///   дают равный [`FormBody`] после реконсиляции пер-форматных денормализаций).
/// * `module` — исходный текст модуля формы (`Module.bsl`), если он есть рядом. `None`, если
///   форма без модуля (9/10 форм корпуса s10_forms).
/// * `help`/`help_resources` — СПРАВКА ФОРМЫ (сайдкар, зеркало объектной справки
///   [`MetadataObject::help`]): EDT `Forms/<F>/Help/<lang>.html`, Designer
///   `Forms/<F>/Ext/Help.xml` + `Ext/Help/<lang>.html` — ровно та же раскладка/кодировки, что у
///   справки ОБЪЕКТА, только якорем служит сама форма (`pipeline::help_read` переиспользуется
///   один-в-один). В cf это ОТДЕЛЬНЫЙ элемент `<form-uuid>.1` (RE ssl.cf: `dump_suffixes`
///   16d44ab1… → `.1` = `{5,1,"ru",{#base64:…}}`, тот же [`HelpPage`]-кодер, что у объектов).
///   Раньше справка формы В IR НЕ ПОПАДАЛА ВООБЩЕ → 354 элемента `<form-uuid>.1` молча не
///   эмитились (доминирующий класс диффов `Справочная информация`).
///
///   NB: у CommonForm справка живёт на ОБЪЕКТЕ ([`MetadataObject::help`]) — объект И ЕСТЬ форма,
///   и её сайдкар лежит по объектному якорю; здесь она НЕ дублируется (см. `form_read`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedFormBody {
    /// Whole opaque ordinary form body, including its embedded module. Exclusive with
    /// a populated managed body or external module; transported verbatim, never rebuilt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinary_body: Option<Vec<u8>>,
    /// Имя формы (идентификатор 1С; для CommonForm == имя объекта).
    pub name: String,
    /// Каноническое тело формы (весь под-IR L1f).
    pub body: FormBody,
    /// Исходный текст модуля формы (`Module.bsl`), если присутствует.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Страницы справки ФОРМЫ (cf `<form-uuid>.1`). Пусто — справки нет.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub help: Vec<HelpPage>,
    /// Файлы-ресурсы справки формы (`Help/_files/…`) — едут ВНУТРИ help-тела.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub help_resources: Vec<HelpResource>,
}

/// КОРНЕВОЙ extInfo формы (`<extInfo xsi:type="form:*FormExtInfo">`).
///
/// EDT кодирует тип формы дискриминатором xsi:type ([`Self::kind`]) и держит СОБСТВЕННЫЕ
/// обработчики формы ([`Self::events`], напр. `AfterWrite` у формы констант) в отдельном
/// блоке `<handlers>` внутри этого extInfo. Designer НЕ несёт маркера типа (он неявен) и
/// сливает эти обработчики в единый корневой `<Events>` вместе с обычными форм-событиями.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormRootExtInfo {
    /// Полное значение xsi:type маркера (напр. `form:ConstantsFormExtInfo`,
    /// `form:ObjectFormExtInfo`, `form:DynamicListFormExtInfo`). EDT-ONLY ⇒ нормализуется ДО X.
    pub kind: String,
    /// СОБСТВЕННЫЕ обработчики формы из extInfo (EDT `<extInfo><handlers>`), в исходном
    /// порядке. Designer сливает их в корневой `<Events>` ⇒ X сравнивает объединённое
    /// множество форм-событий.
    pub events: Vec<FormEvent>,
    /// Группа пользовательских настроек формы КОМПОНОВЩИКА НАСТРОЕК (не-отчётной): EDT
    /// `<extInfo xsi:type="form:SettingsComposerFormExtInfo"><userSettingsGroup>` ⟺ Designer
    /// корневой `<CustomSettingsFolder>` (witness Catalog.УчетныеЗаписиМаркетплейсов/
    /// DataProcessor.ИнтерфейсДокументовЭДО .НастройкаОтборовСписка — основной реквизит
    /// `DataCompositionSettingsComposer`). ОБА формата несут ⇒ X-сравним. У ФОРМЫ ОТЧЁТА эта
    /// группа живёт в [`ReportFormInfo::user_settings_group`] — здесь `None`. `None` ⇒ тег отсутствует.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_settings_group: Option<String>,
}

/// Форм-уровневая информация формы отчёта (`form:ReportFormExtInfo` EDT ⟺ набор
/// корневых элементов Designer). ОБА формата несут её содержимое (X-сравнимо), но кодируют
/// по-разному: EDT держит поля ВНУТРИ корневого `<extInfo xsi:type="form:ReportFormExtInfo">`
/// (тегами `settingsForm`/`showState`/`userSettingsGroup`/`reportResult`/
/// `viewModeApplicationOnSetReportResult`), Designer — прямыми детьми корня `<Form>`
/// (`ReportFormType`/`AutoShowState`/`CustomSettingsFolder`/`ReportResultViewMode`/
/// `ViewModeApplicationOnSetReportResult`).
///
/// KEEP-поля (`show_state`/`report_result_view_mode`/`view_mode_application`) несут ПЕР-ФОРМАТНЫЕ
/// дефолты: Designer эмитит их ВСЕГДА (в т.ч. `Auto`), EDT ОПУСКАЕТ `Auto`. Оба ридера
/// заполняют дефолт `Auto` при отсутствии ⇒ канон X-равен.
///
/// `settings_form` (дискриминатор вида формы отчёта — `Main`/`Variant`/`Settings`): Designer эмитит
/// `ReportFormType` ВСЕГДА; EDT эмитит `<settingsForm>` только для `Variant`/`Settings` (тип `Main`
/// НЕЯВЕН — кодируется ОТСУТСТВИЕМ `settingsForm`). Форма отчёта типа `Main` дополнительно несёт
/// поля-ссылки `report_result`/`details_data`/`variant_appearance` (Designer прямыми детьми корня;
/// EDT — тегами внутри extInfo). Все три X-сравнимы напрямую (оба формата несут). `user_settings_group`
/// — оба эмитят при наличии (иначе опускают).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportFormInfo {
    /// Вид формы отчёта (`settingsForm` EDT ⟺ `ReportFormType` Designer): `Main`/`Variant`/
    /// `Settings`. EDT опускает `<settingsForm>` для `Main` (тип неявен отсутствием).
    pub settings_form: String,
    /// Автопоказ состояния (`showState` ⟺ `AutoShowState`), дефолт `Auto`.
    pub show_state: String,
    /// Группа пользовательских настроек (`userSettingsGroup` ⟺ `CustomSettingsFolder`),
    /// если задана.
    pub user_settings_group: Option<String>,
    /// Поле-результат табличного документа (`reportResult` EDT ⟺ `ReportResult` Designer) —
    /// форма отчёта `Main`. `None` у `Variant`/`Settings`. X-сравнимо напрямую.
    #[serde(default)]
    pub report_result: Option<String>,
    /// Native legacy xs:decimal identifier, paired with its resolved name.
    #[serde(skip)]
    pub designer_report_result_id: Option<(String, String)>,
    /// EDT source identifier spelling, paired with its resolved name.
    #[serde(skip)]
    pub edt_report_result_id: Option<(String, String)>,
    /// Поле данных расшифровки (`detailsInformation` EDT ⟺ `DetailsData` Designer) — `Main`.
    /// `None` у `Variant`/`Settings`. X-сравнимо напрямую.
    #[serde(default)]
    pub details_data: Option<String>,
    #[serde(skip)]
    pub designer_details_data_id: Option<(String, String)>,
    #[serde(skip)]
    pub edt_details_data_id: Option<(String, String)>,
    /// Поле наименования текущего варианта (`currentVariantPresentationField` EDT ⟺
    /// `VariantAppearance` Designer) — `Main`. `None` у `Variant`/`Settings`. X-сравнимо напрямую.
    #[serde(default)]
    pub variant_appearance: Option<String>,
    /// Режим показа результата (`reportResultViewMode` ⟺ `ReportResultViewMode`), дефолт `Auto`.
    #[serde(default = "report_form_auto")]
    pub report_result_view_mode: String,
    /// Применение режима показа при установке результата
    /// (`viewModeApplicationOnSetReportResult` ⟺ `ViewModeApplicationOnSetReportResult`),
    /// дефолт `Auto`.
    pub view_mode_application: String,
}

/// serde-дефолт `Auto` для [`ReportFormInfo::report_result_view_mode`].
fn report_form_auto() -> String {
    REPORT_FORM_AUTO.to_string()
}

/// Канонический литерал `Auto` KEEP-полей формы отчёта (EDT опускает, Designer эмитит).
pub const REPORT_FORM_AUTO: &str = "Auto";

/// Параметр формы (`<parameters>`/`<Parameter>`): имя + тип значения + ключевой флаг.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormParameter {
    /// Имя параметра.
    pub name: String,
    /// Тип значения параметра (`None` = present-empty `<valueType/>`/`<Type/>`).
    pub value_type: Option<crate::ir::TypeSpec>,
    /// Ключевой параметр (`<keyParameter>true`/`<KeyParameter>true`); оба эмитят.
    pub key_parameter: bool,
}

/// Пользовательская КОМАНДА формы (`<formCommands>` EDT / `<Commands><Command>` Designer).
///
/// Идентичность — `(name, id)` (EDT — `<name>`/`<id>`-дети, Designer — `name=`/`id=`-
/// атрибуты). Прочие свойства — property-bag по каноническому [`FieldId`] спека
/// `spec/forms/command` (§1.6): title/toolTip/shortcut/picture/action/actionPurpose/
/// representation/modifiesStoredData/currentRowUse/associatedTableElementId/
/// selectedRowsUse, в каноническом порядке спека, дефолты не хранятся (кроме
/// required-полей `currentRowUse`/`selectedRowsUse` — их пер-форматные дефолты
/// ПРОТИВОПОЛОЖНЫ, ридер заполняет дефолт СВОЕГО формата ⇒ X-равенство структурно).
///
/// Формат-константы (EDT `<use><common>true</common></use>`, Designer
/// `<xr:LoadTransparent>` картинки) в IR НЕ хранятся — реконструируются каркасом.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormCommand {
    /// Имя команды (уникально в форме).
    pub name: String,
    /// Числовой `id` команды (форм-локальный, общий обоим форматам).
    pub id: i64,
    /// Свойства команды по каноническому `FieldId` (`spec/forms/command`), в порядке
    /// эмиссии спека. `Vec`, а не map — порядок структурен (§1.3).
    #[serde(serialize_with = "crate::ir::serialize_semantic_properties")]
    pub properties: Vec<(FieldId, PropertyValue)>,
}

impl FormCommand {
    /// Создать команду с пустым property-bag.
    pub fn new(name: impl Into<String>, id: i64) -> Self {
        FormCommand {
            name: name.into(),
            id,
            properties: Vec::new(),
        }
    }

    /// Значение свойства по каноническому id, если присутствует.
    pub fn get(&self, id: FieldId) -> Option<&PropertyValue> {
        self.properties
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v)
    }
}

impl FormBody {
    /// Создать пустое тело формы.
    pub fn new() -> Self {
        FormBody {
            designer_checkbox_auto_presence: std::collections::BTreeMap::new(),
            common_picture_transparency: std::collections::BTreeMap::new(),
            designer_path_spelling: false,
            ca_envelope_without_lf_pal: false,
            title: None,
            attributes: Vec::new(),
            events: Vec::new(),
            items: Vec::new(),
            auto_command_bar: None,
            command_interface: false,
            form_ci_navigation_panel: Vec::new(),
            form_ci_command_bar: Vec::new(),
            create_buttons_group_title: None,
            data_attributes: Vec::new(),
            conditional_appearance: Vec::new(),
            parameters: Vec::new(),
            commands: Vec::new(),
            excluded_commands: Vec::new(),
            root_ext_info: None,
            report_form: None,
            mobile_device_command_bar: Vec::new(),
            use_for_folders_and_items: None,
            document_form: None,
        }
    }

    /// Значение форм-уровневого свойства по каноническому id, если присутствует.
    pub fn get(&self, id: FieldId) -> Option<&PropertyValue> {
        self.attributes
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v)
    }
}

impl Default for FormBody {
    fn default() -> Self {
        Self::new()
    }
}
