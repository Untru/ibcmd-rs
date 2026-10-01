use super::*;

/// serde-дефолт `true` (view/edit-права реквизита).
fn bool_true() -> bool {
    true
}

/// ДАННЫЙ-реквизит формы (`<attributes>`/`<Attribute>`): имя + id + тип значения +
/// частые флаги (LANE-F-2).
///
/// `value_type` — описание типа 1С ([`crate::ir::TypeSpec`]); `None` = пустой тип
/// (`<valueType/>`/`<Type/>` — типизированный-но-пустой). `view`/`edit` — флаги
/// `<view><common>`/`<edit><common>` (EDT-only denormalized presence; X-исключены).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormDataAttribute {
    /// Имя реквизита.
    pub name: String,
    /// `id` реквизита.
    pub id: i64,
    /// Локализованный заголовок (`<title>`/`<Title>`), если задан. Оба формата эмитят.
    pub title: Option<PropertyValue>,
    /// Тип значения (`None` = present-empty `<valueType/>`).
    pub value_type: Option<crate::ir::TypeSpec>,
    /// Контроль заполнения (`<fillChecking>`/`<FillCheck>`, напр. `ShowError`); оба эмитят.
    pub fill_checking: Option<String>,
    /// Право ПРОСМОТРА (`<view>` EDT ⟺ `<View>` Designer): `true` = общий просмотр (EDT
    /// эмитит `<view><common>true</common></view>`; Designer тега НЕ несёт); `false` =
    /// запрещён (EDT `<view/>` ПУСТОЙ; Designer `<View><xr:Common>false</xr:Common></View>`
    /// после `<Type>`). Витнессы УчетныеЗаписиЭлектроннойПочты.ПомощникНастройки ×2.
    /// ОБА формата несут ⇒ X-сравним.
    #[serde(default = "bool_true")]
    pub view_common: bool,
    /// Право РЕДАКТИРОВАНИЯ (`<edit>` ⟺ `<Edit>`) — та же кодировка, что [`Self::view_common`]
    /// (витнессы Взаимодействия.ФормаСписка, ВерсииОбъектов колонка НомерВерсии).
    #[serde(default = "bool_true")]
    pub edit_common: bool,
    /// РОЛЕВЫЕ права ПРОСМОТРА: `(роль, значение)` в исходном порядке; пусто = безролевой
    /// View (witness ERP W11 БухучетЗарплатыОрганизаций.РедактированиеИстории:
    /// Common=false + xr:Value-роли ⟷ EDT `<for><value><role>`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub view_roles: Vec<(String, bool)>,
    /// РОЛЕВЫЕ права РЕДАКТИРОВАНИЯ (зеркально [`Self::view_roles`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edit_roles: Vec<(String, bool)>,
    /// Основной реквизит (`<main>true`/`<MainAttribute>true`); оба эмитят.
    pub main: bool,
    /// Сохраняемые данные (`<savedData>true`/`<SavedData>true`); оба эмитят.
    pub saved_data: bool,
    /// Сохранение настроек: пути данных — СПИСОК (реквизит может нести несколько; witness
    /// ГрупповоеИзменениеРеквизитов ×2): EDT — ПОВТОРЯЕМЫЕ `<settingsSavedData
    /// xsi:type="form:DataPath"><segments>Путь</segments>`; Designer — ОДИН `<Save>` с
    /// несколькими `<Field>Путь</Field>`. Порядок сохраняется. X-сравним (оба формата несут
    /// один список). Пусто ⇒ регион отсутствует.
    #[serde(default)]
    pub settings_saved_data: Vec<String>,
    /// DESIGNER-ONLY: КАНОНИЧЕСКИЕ пути (из [`Self::settings_saved_data`] /
    /// [`Self::not_default_use_always`]), которые Designer пометил сигилой `~` — «поле не входит в
    /// состав полей динамического списка»: `<UseAlways><Field>~Список.Ограничения</Field>`.
    ///
    /// Тот же сигил-механизм, что у [`FormItem::row_picture_path_unavailable`], но здесь он
    /// ПОЭЛЕМЕНТНЫЙ (в одном `<UseAlways>` часть путей помечена, часть — нет), поэтому носитель —
    /// МНОЖЕСТВО помеченных путей, а не bool. EDT-корпус сигилу не несёт НИ РАЗУ, и edt→cf
    /// byte-exact без неё ⇒ в cf-тело она НЕ кодируется: чисто designer-локальная денормализация.
    /// ⇒ КАНОН = EDT-написание (пути без `~`), сигил держится этим множеством: `read_designer`
    /// снимает и запоминает, `write_designer` возвращает, `normalize_attr_for_x` зануляет.
    /// EDT-сторона его не несёт (пусто).
    #[serde(skip)]
    pub designer_unavailable_paths: Vec<String>,
    /// КОЛОНКИ табличного реквизита (`ValueTable`): EDT — повторяемые `<columns>`-дети
    /// `<attributes>`; Designer — `<Columns><Column name id>` под `<Attribute>`. Рекурсивно
    /// [`FormDataAttribute`] (колонка = имя+id+title+тип). Пусто у скалярных реквизитов.
    /// X-сравнимы (оба формата несут).
    #[serde(default)]
    pub columns: Vec<FormDataAttribute>,
    /// ДОПОЛНИТЕЛЬНЫЕ КОЛОНКИ табличного реквизита (`ValueTable`) — группы колонок,
    /// привязанные к вложенному табличному пути: EDT — ПОВТОРЯЕМЫЕ `<additionalColumns>`
    /// (сиблинги `<columns>`, каждый несёт `<tablePath xsi:type="form:DataPath"><segments>` +
    /// вложенные `<columns>`); Designer — `<AdditionalColumns table="Путь">` ВНУТРИ `<Columns>`
    /// (после обычных `<Column>`, каждый несёт `<Column>`-детей). Порядок групп и колонок значим.
    /// Пусто у реквизитов без доп-колонок. X-сравнимы (оба формата несут table_path+columns).
    #[serde(default)]
    pub additional_columns: Vec<AdditionalColumns>,
    /// Несёт ли реквизит `ValueList`-extInfo — маркер типа «Список значений»: EDT
    /// `<extInfo xsi:type="form:ValueListExtInfo"><itemValueType/></extInfo>` ⟺ Designer
    /// `<Settings xsi:type="v8:TypeDescription"/>` (оба — ПУСТЫЕ маркеры при `valueType`
    /// = ValueList; при `None` `itemValueType`/`Settings`-контент пуст). X-сравним (presence-бит).
    #[serde(default)]
    pub value_list_ext: bool,
    /// Тип элементов списка значений при `value_list_ext == true`: EDT НЕ-пустой
    /// `<itemValueType><types>String</types>…</itemValueType>` ⟺ Designer `<Settings
    /// xsi:type="v8:TypeDescription"><v8:Type>xs:string</v8:Type>…`. Оба формата несут ОДИН
    /// [`TypeSpec`](crate::ir::TypeSpec) (через общий type-codec) ⇒ X-сравним НАПРЯМУЮ (симметрично,
    /// НЕ нормализуется — расхождение типа элементов ⇒ X=FALSE). `None` ⇒ пустой маркер
    /// `<itemValueType/>` (совместимо со всем прежним корпусом, где item-тип пуст).
    #[serde(default)]
    pub value_list_item_type: Option<crate::ir::TypeSpec>,
    /// Несёт ли реквизит EDT-маркер `<extInfo xsi:type="form:SpreadsheetDocumentExtInfo"/>`
    /// — ПОЛНОСТЬЮ пустой (без внутреннего `<itemValueType/>`, в отличие от
    /// [`Self::value_list_ext`]). Маркер ПОЛНОСТЬЮ детерминирован типом: скаляр-тип
    /// `SpreadsheetDocument` ⟺ маркер (37/37 корпус SSL+coverage; у составных типов, включающих
    /// `SpreadsheetDocument`, маркера НЕТ). Физически несёт лишь EDT (Designer — только
    /// `<Type>`), но поле РЕКОНСИЛИРУЕМО: EDT-ридер сверяет детерминант (§1.0),
    /// Designer-ридер ВЫВОДИТ маркер из типа ⇒ designer→edt восстанавливает его byte-exact.
    #[serde(default)]
    pub spreadsheet_ext: bool,
    /// Функциональные опции реквизита (`<functionalOptions>Ref</functionalOptions>` EDT,
    /// повторяемый ⟺ `<FunctionalOptions><Item>Ref</Item></FunctionalOptions>` Designer) —
    /// список ссылок на функциональные опции в исходном порядке. Пусто ⇒ регион отсутствует.
    /// X-сравнимы (оба формата несут).
    #[serde(default)]
    pub functional_options: Vec<String>,
    /// Пути реквизита «использовать всегда, не по умолчанию» — СПИСОК (реквизит может нести
    /// несколько): EDT — ПОВТОРЯЕМЫЕ `<notDefaultUseAlwaysAttributes xsi:type="form:DataPath">
    /// <segments>Путь</segments>`; Designer — ОДИН `<UseAlways>` с несколькими `<Field>Путь</Field>`.
    /// Порядок сохраняется. X-сравним (оба формата несут один список). Пусто ⇒ регион отсутствует.
    #[serde(default)]
    pub not_default_use_always: Vec<String>,
    /// extInfo ДАННЫХ-реквизита динамического списка (`form:DynamicListExtInfo` EDT ⟺
    /// `<Settings xsi:type="DynamicList">` Designer), если реквизит — динамический список
    /// (тип `DynamicList`). `None` ⇒ обычный реквизит. См. [`DynamicListAttrExt`].
    #[serde(default)]
    pub dynamic_list: Option<DynamicListAttrExt>,
    /// Настройки диаграммы Chart/GanttChart-реквизита, если несёт (см. [`ChartSettings`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart_settings: Option<ChartSettings>,
    /// ТЕЛО табличного документа реквизита — ФОРМАТ-НЕЙТРАЛЬНЫЙ под-IR двух сериализаций
    /// (содержимое сверено 1:1, 11/11 пар корпуса SSL+coverage): Designer — ИНЛАЙН
    /// `<Settings xmlns:mxl=… xsi:type="mxl:SpreadsheetDocument">` в `Ext/Form.xml` (читает/пишет
    /// форм-кодек); EDT — САЙДКАР `Attributes/<attr>/ExtInfo/SpreadsheetData.mxlx` рядом с
    /// `Form.form` (в самом `Form.form` — лишь пустой маркер [`Self::spreadsheet_ext`];
    /// транскодит pipeline `form_read`/`form_write`). Смоделирован СТРУКТУРНО (§1.0 запрещает
    /// Blob); незнакомая структура ⇒ ошибка. На КОДЕК-уровне X-нормализуется (EDT-кодек сайдкара
    /// не видит — он приходит pipeline-attach'ем). `None` ⇒ реквизит без сохранённого тела
    /// (валидно и ПРИ маркере: 26/36 SSL-маркеров тела не несут). См. [`MxlSpreadsheetSettings`].
    #[serde(default)]
    pub spreadsheet_settings: Option<MxlSpreadsheetSettings>,
}

/// Тело табличного документа реквизита формы — общий под-IR ДВУХ сериализаций: Designer-инлайн
/// `<Settings xsi:type="mxl:SpreadsheetDocument">` (`mxl:`-префикс) и EDT-сайдкар
/// `SpreadsheetData.mxlx` (default-ns `<document>`). Смоделирован пустой табличный документ по умолчанию:
/// `<mxl:columns><mxl:size>N</mxl:size></mxl:columns>`, `<mxl:rowsItem><mxl:index>N</mxl:index>
/// <mxl:row><mxl:empty>b</mxl:empty></mxl:row></mxl:rowsItem>`, `<mxl:vgRows>N</mxl:vgRows>`. §1.0:
/// незнакомый под-элемент/структура ⇒ ошибка ридера (не Blob, не skip). Нормализуется ДО X.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MxlSpreadsheetSettings {
    /// ERP-флавор конверта `.mxlx`: корень `<document>` БЕЗ `xmlns:pal` (ценз ERP 18/18; SSL несёт
    /// pal 10/10). Presence-бит для byte-exact re-emit сайдкара; `false` = SSL-флавор. Чисто
    /// EDT-сайдкар-концерн: `normalize_attr_for_x` зануляет весь `spreadsheet_settings` до X, а
    /// cf-Moxel строится из СТРУКТУРНЫХ полей (pal не читает) — так что бит невидим X и cf.
    #[serde(default)]
    pub envelope_without_pal: bool,
    /// ОПЦИОНАЛЬНЫЙ префикс `<mxl:languageSettings>` (языки табличного документа). Присутствует у
    /// непустых/локализованных документов (RE: SSL CommonForm.РедактированиеТабличногоДокумента);
    /// отсутствует у минимального пустого (coverage). `None` ⇒ блок опущен.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_settings: Option<MxlLanguageSettings>,
    /// `<mxl:columns><mxl:size>N` — размер колонок (текст, обычно `0`).
    pub columns_size: String,
    /// `<mxl:rowsItem><mxl:index>N` — индекс строки (текст, обычно `0`).
    pub rows_index: String,
    /// `<mxl:rowsItem><mxl:row><mxl:empty>b` — пустая строка (обычно `true`).
    pub row_empty: bool,
    /// ОПЦИОНАЛЬНЫЙ `<mxl:templateMode>b` (между rowsItem и vgRows) — режим макета. Присутствует у
    /// макетных таб.документов (RE: РедактированиеТабличногоДокумента); `None` ⇒ опущен.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_mode: Option<bool>,
    /// `<mxl:vgRows>N` — число зафиксированных строк (текст, обычно `0`).
    pub vg_rows: String,
    /// БОГАТОЕ тело табличного документа — ПОЛНЫЙ generic-tree документа (живой заполненный
    /// SpreadsheetDocument: `columnsItem`/ячейки-`c` с `tl`-заголовками/`namedItem`/`merge`/`area`/
    /// `font`/`format`/`line`/`defaultFormatIndex`/`height`/… — вне минимального конверта). `Some` ⇒
    /// БОГАТЫЙ вариант: верхнеуровневые поля выше (`columns_size`/`rows_index`/…) — САНТИНЕЛИ и НЕ
    /// используются; всё тело несёт этот вектор верхнеуровневых узлов `<document>`/`<Settings>` в
    /// ИСХОДНОМ порядке и с ИСХОДНЫМ диалектным написанием (EDT-сайдкар: default-ns, атрибуты
    /// алфавитно, инлайн-редекл `xmlns:v8`/`xmlns:v8ui`; Designer-инлайн: `mxl:`-префикс, атрибуты
    /// семантически, без редекла). `None` ⇒ МИНИМАЛЬНЫЙ вариант (пустой документ — структурные поля
    /// выше, byte-exact путь SSL/ERP-минимума не тронут).
    ///
    /// §1.0: НЕ Blob/passthrough — тело РАЗОБРАНО в structural [`MxlNode`]-дерево (каждый узел/атрибут
    /// склеймлен на чтении, тотальность соблюдена) и РЕГЕНЕРИРУЕТСЯ byte-exact на записи; неизвестных
    /// «сырых» кусков нет. Диалектное написание ХРАНИТСЯ пер-источник ⇒ same-dialect R байт-точен без
    /// знания per-element схемы. cf-Moxel богатого тела НЕ витнесснут (общий non-template MXL→Moxel
    /// сериализатор отсутствует — `formats_cf::mxl_body` только для макетов `templateMode=true`) ⇒
    /// cf-эмиссия ГРОМКО отказывает (§1.0), а не пишет неверный Moxel. X-нормализация зануляет весь
    /// `spreadsheet_settings` (см. `normalize_attr_for_x`) ⇒ EDT-флейвор и Designer-флейвор дерева
    /// X-невидимы.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_body: Option<Vec<MxlNode>>,
}

/// Один узел generic-tree богатого табличного документа реквизита (см.
/// [`MxlSpreadsheetSettings::full_body`]). Полный структурный слепок XML-элемента —
/// префикс+local+атрибуты(в исходном порядке, включая инлайн-`xmlns:*`)+текст(листа)+дети —
/// достаточный для byte-exact ре-эмиссии В ТОМ ЖЕ диалекте. §1.0: не Blob — всё дерево разобрано и
/// склеймлено; неизвестных фрагментов нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MxlNode {
    /// Ns-префикс тега (`""` = default-ns/без префикса; `mxl`/`v8`/`v8ui`/…).
    pub prefix: String,
    /// Local-name тега.
    pub local: String,
    /// Атрибуты `(имя, значение)` в ИСХОДНОМ порядке (значения unescaped; включает инлайн-`xmlns:*`
    /// и `xsi:type`). Порядок ХРАНИТСЯ — он диалектно-специфичен (EDT алфавитный, Designer
    /// семантический) и воспроизводится дословно.
    pub attrs: Vec<(String, String)>,
    /// Дочерние узлы в исходном порядке (пусто у листа/самозакрытого).
    #[serde(default)]
    pub children: Vec<MxlNode>,
    /// Текст листа (unescaped; `""` у ветки/самозакрытого).
    #[serde(default)]
    pub text: String,
    /// Эмитить как самозакрывающийся `<tag/>` (пустой без детей/текста — конвенция MXL/1С).
    #[serde(default)]
    pub self_closing: bool,
}

/// `<mxl:languageSettings>` — языковые настройки табличного документа (текущий/по умолчанию язык +
/// список `<mxl:languageInfo>`). RE: SSL CommonForm.РедактированиеТабличногоДокумента.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MxlLanguageSettings {
    /// `<mxl:currentLanguage>` — код текущего языка (напр. `ru`).
    pub current_language: String,
    /// `<mxl:defaultLanguage>` — код языка по умолчанию (напр. `ru`).
    pub default_language: String,
    /// `<mxl:languageInfo>` в исходном порядке (обычно один).
    pub languages: Vec<MxlLanguageInfo>,
}

/// Одна запись `<mxl:languageInfo>` — язык табличного документа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MxlLanguageInfo {
    /// `<mxl:id>` — код языка (`ru`).
    pub id: String,
    /// `<mxl:code>` — представление кода (`Русский`).
    pub code: String,
    /// `<mxl:description>` — описание (`Русский`).
    pub description: String,
}

/// Группа ДОПОЛНИТЕЛЬНЫХ КОЛОНОК табличного реквизита (`ValueTable`), привязанная к
/// вложенному табличному пути.
///
/// # Асимметрия локусов (§1.6)
/// EDT — `<additionalColumns>` (ПОВТОРЯЕМЫЙ сиблинг `<columns>` внутри `<attributes>`), несёт
/// `<tablePath xsi:type="form:DataPath"><segments>Путь</segments></tablePath>` + вложенные
/// `<columns>`-дети. Designer — `<AdditionalColumns table="Путь">` ВНУТРИ `<Columns>` (после
/// обычных `<Column>`), несёт `<Column>`-детей. ОБА формата несут [`Self::table_path`] и
/// [`Self::columns`] ⇒ X-сравнимо НАПРЯМУЮ (не нормализуется).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdditionalColumns {
    /// Табличный путь группы (EDT `<tablePath><segments>` ⟺ Designer `table=`), напр.
    /// `ТаблицаВопросовРаздела.СоставТабличногоВопроса`. X-сравним.
    pub table_path: String,
    /// Колонки группы (EDT вложенные `<columns>` ⟺ Designer `<Column>`), рекурсивно
    /// [`FormDataAttribute`] (колонка = имя+id+title+тип), в исходном порядке. X-сравнимы.
    pub columns: Vec<FormDataAttribute>,
}

/// extInfo ДАННЫХ-реквизита динамического списка (`form:DynamicListExtInfo` EDT ⟺
/// `<Settings xsi:type="DynamicList">` Designer).
///
/// # Асимметрия локусов (§1.6)
/// * ОБЩЕЕ ЯДРО (X-сравнимо, оба формата несут): текст запроса ([`Self::query_text`]),
///   основная таблица ([`Self::main_table`]), произвольный запрос ([`Self::custom_query`] ⟺
///   Designer `ManualQuery`), динамическое чтение ([`Self::dynamic_data_read`]).
/// * EDT-ONLY флаги ([`Self::auto_fill_available_fields`]/[`Self::auto_save_user_settings`]/
///   [`Self::get_invisible_field_presentations`]) — Designer их НЕ несёт (в `.cf`/Designer это
///   DCS-дефолты, опущены). Нужны для EDT R byte-exact; нормализуются ДО X (`normalize_form_for_x`).
/// * DESIGNER-ONLY [`Self::list_settings`] (`<ListSettings>` dcsset) — DCS user-settings,
///   которые EDT держит в САЙДКАРЕ `Attributes/<Имя>/ExtInfo/ListSettings.dcss` (НЕ в `Form.form`).
///   Нужны для Designer R byte-exact; нормализуются ДО X (EDT `Form.form` аналога не несёт).
///
///
/// Настройки диаграммы форм-реквизита (Designer `<Settings xsi:type="d4p1:Chart|GanttChart">`
/// ⟷ EDT сайдкар `Attributes/<attr>/ExtInfo/Chart.chart|GanttChart.chart` ⟷ cf ext-клетка
/// `{"#",3543ef08…/3a6e63bf…}`; ERP-witnessed 9 блоков в 5 формах, W17-линия).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSettings {
    /// `"Chart"` | `"GanttChart"`.
    pub kind: String,
    /// Поля верхнего уровня в порядке источника (имена — models/charts; валидация в кодеках).
    pub fields: Vec<(String, ChartValue)>,
}

/// Значение поля диаграммы (числа/decimal — ЛЕКСИЧЕСКИЕ String: byte-exact без формат-риска).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChartValue {
    /// Булево.
    Bool(bool),
    /// Целое (лексически).
    Int(String),
    /// Строка.
    Str(String),
    /// Enum-литерал.
    Enum(String),
    /// Цвет (канон-текст: style:/web:/абсолютный).
    Color(String),
    /// Шрифт.
    Font(FontRef),
    /// Линия (стиль+толщина+gap).
    Line {
        /// Стиль линии.
        style: String,
        /// Толщина (лексически).
        width: String,
        /// Просвет.
        gap: bool,
    },
    /// Рамка (стиль+толщина).
    Border {
        /// Стиль рамки.
        style: String,
        /// Толщина (лексически).
        width: String,
    },
    /// Локализованная строка.
    Localized(Vec<(Lang, String)>),
    /// Прямоугольник (лексические координаты).
    Rect {
        /// Левая граница.
        left: String,
        /// Правая граница.
        right: String,
        /// Верхняя граница.
        top: String,
        /// Нижняя граница.
        bottom: String,
    },
    /// Повторяемые вставные блоки (series/points/dataItems…).
    Items(Vec<Vec<(String, ChartValue)>>),
    /// Вложенный композит (axis/scale/area…).
    Nested(Vec<(String, ChartValue)>),
}

/// Порядок эмиссии РАСХОДИТСЯ: EDT — queryText, mainTable, флаги; Designer — ManualQuery,
/// DynamicDataRead, QueryText, MainTable, ListSettings. Каждый writer восстанавливает свой.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicListAttrExt {
    /// Текст запроса (`queryText` ⟺ `QueryText`). X-сравним. `None` ⇒ АВТО-запрос: платформа
    /// выводит запрос из `main_table` (ManualQuery=false); тег `queryText`/`QueryText` физически
    /// отсутствует у обоих форматов. `Some` ⇒ РУЧНОЙ запрос (`custom_query == true`).
    #[serde(default)]
    pub query_text: Option<String>,
    /// Основная таблица (`mainTable` ⟺ `MainTable`), напр. `Catalog.Пользователи`. X-сравним.
    /// `None` ⇒ РАСШИРЕННАЯ форма (собственный DataCompositionSchema data-set через `fields`/
    /// `parameters` без единой основной таблицы); тег отсутствует у обоих форматов.
    #[serde(default)]
    pub main_table: Option<String>,
    /// Тип ключа динамического списка (`keyType` ⟺ `KeyType`; ERP-witnessed 7 файлов).
    /// `None` ⇒ отсутствует.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_type: Option<String>,
    /// Произвольный (ручной) запрос (`customQuery` ⟺ `ManualQuery`). EDT: presence-bool (эмитит
    /// `true`, опускает `false`). Designer: ВСЕГДА эмитит явный `true`/`false`. `false` ⇒ авто-
    /// запрос (`query_text == None`). X-сравним.
    pub custom_query: bool,
    /// Динамическое чтение данных (`dynamicDataRead` ⟺ `DynamicDataRead`). EDT: presence-bool
    /// (эмитит `true`, опускает `false`). Designer: ВСЕГДА эмитит явный `true`/`false`. X-сравним.
    pub dynamic_data_read: bool,
    /// Автозаполнение доступных полей (`autoFillAvailableFields` ⟺ `AutoFillAvailableFields`) —
    /// ДАННЫЕ с ПЕР-ФОРМАТНЫМИ дефолтами (witness АнализПравДоступа.ВыборСтрокиРегистра):
    /// EDT-дефолт `false` (эмитит `<autoFillAvailableFields>true`), Designer-дефолт `true`
    /// (эмитит `<AutoFillAvailableFields>false` ПЕРВЫМ ребёнком Settings). Историческая
    /// X-нормализация сохранена (testkit), но ридеры/райтеры симметричны по значению.
    #[serde(default)]
    pub auto_fill_available_fields: bool,
    /// Автосохранение пользовательских настроек (`autoSaveUserSettings` ⟺ `AutoSaveUserSettings`)
    /// — те же пер-форматные дефолты (witness ГрупповоеИзменениеРеквизитов.ВыбранныеЭлементы):
    /// EDT эмитит `true`, Designer эмитит `false` (после KeyField/Parameter, до MainTable).
    #[serde(default)]
    pub auto_save_user_settings: bool,
    /// EDT-only: получать представления невидимых полей (`getInvisibleFieldPresentations`).
    /// Designer НЕ несёт (0 вхождений корпуса; 229/229 EDT-динсписков несут `true`) ⇒
    /// Designer-ридер канонизует `true`. Норм. ДО X.
    #[serde(default)]
    pub get_invisible_field_presentations: bool,
    /// Ключевые поля динамического списка (`keyField` ⟺ `KeyField`, ПОВТОРЯЕМЫЙ) — напр.
    /// `Ссылка` у списка с произвольным запросом без основной таблицы (witness
    /// ГрупповоеИзменениеРеквизитов.ВыбранныеЭлементы, ×1), либо СПИСОК ключей у ERP-списков с
    /// собственной DCS-схемой (witness ЖурналДокументовПрослеживаемости.СписокДокументов ×4,
    /// МониторингЗаказаНормативныйГрафик.ФормаОтчета ×7). ОБА формата несут (в исходном порядке,
    /// ПОСЛЕ `fields`/`parameters`/`keyType`) ⇒ X-сравнимы напрямую. Пусто ⇒ тег отсутствует.
    #[serde(default)]
    pub key_fields: Vec<String>,
    /// Вычисляемые поля динамического списка (EDT `<calculatedFields>` ⟺ Designer
    /// `<CalculatedField>`; witness НеудаленныеОбъекты.ФормаСписка). ОБА формата несут ⇒
    /// X-сравнимы. См. [`DcsCalculatedField`].
    #[serde(default)]
    pub calculated_fields: Vec<DcsCalculatedField>,
    /// Designer-only DCS-настройки списка (`<ListSettings>`); `None` у EDT-стороны. Норм. ДО X.
    #[serde(default)]
    pub list_settings: Option<DcsListSettings>,
    /// Поля схемы набора данных DCS (EDT `<fields xsi:type="schema:DataCompositionSchemaDataSetField">`
    /// ⟺ Designer `<Field xsi:type="dcssch:DataSetFieldField">`) — EXTENDED-форма динсписка. ОБА
    /// формата несут ⇒ X-сравнимы НАПРЯМУЮ. Пусто ⇒ SIMPLE-форма (без явной схемы). См. [`DcsField`].
    #[serde(default)]
    pub fields: Vec<DcsField>,
    /// Параметры схемы набора данных DCS (EDT `<parameters>` ⟺ Designer `<Parameter>`) —
    /// EXTENDED-форма. ОБА формата несут ⇒ X-сравнимы. Пусто ⇒ без явных параметров. См. [`DcsParameter`].
    #[serde(default)]
    pub parameters: Vec<DcsParameter>,
}
