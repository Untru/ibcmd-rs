use super::*;

/// Вид контрола формы — канонический, формат-нейтральный идентификатор типа узла
/// (`InputField`, `Table`, `LabelDecoration`, `UsualGroup`, …).
///
/// Хранится строкой-кодом, чтобы каталог контролов рос без правки IR (новый контрол =
/// новый спек в `spec/forms/controls`), оставаясь при этом одним и тем же значением во
/// всех форматах (§1.6). Это КАНОНИЧЕСКИЙ вид — он РАЗРЕШАЕТ форматную дивергенцию
/// дискриминатора (EDT кодирует вид как `xsi:type="form:Decoration"` + `<type>Label</type>`;
/// Designer — как имя элемента `<LabelDecoration>`); канон — один (`LabelDecoration`),
/// каждый формат восстанавливает свою кодировку через проекцию.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FormControlKind(pub String);

impl FormControlKind {
    /// Построить вид контрола из канонического кода.
    pub fn new(s: impl Into<String>) -> Self {
        FormControlKind(s.into())
    }
    /// Код вида как `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Совместимый алиас прежнего имени (`ControlKind`) — переиспользуется тестами/зеркалами.
pub type ControlKind = FormControlKind;

/// ИНЛАЙН-картинка контрола формы: байты БИНАРНОГО файла-спутника `Items/<ctl>/<Tag>.<ext>`.
///
/// §1.0 Blob — опаковый платформенный бинарь: читается ВЕРБАТИМ, пишется ВЕРБАТИМ, без всякой
/// интерпретации zip/png/svg-внутренностей. Дескриптор формы несёт лишь МАРКЕР того, что
/// картинка — спутниковая, а НЕ ссылка на метаданные:
/// * EDT — пустой `<tag xsi:type="form:FormPicture"/>` (расширение НЕ хранится — оно у файла);
/// * Designer — `<tag><xr:Abs><Tag>.<ext></xr:Abs><xr:LoadTransparent>false</…></tag>`.
///
/// Оба формата держат ОДИН И ТОТ ЖЕ файл (сверено: 19/19 SSL-спутников байт-идентичны между
/// EDT и Designer) ⇒ канон `Ref("abs:<ext>")` + эти байты X-сравнимы ПО ПОСТРОЕНИЮ (§1.6).
/// Сами байты живёт читать pipeline'у (как `SpreadsheetData.mxlx`/`ListSettings.dcss`), а НЕ
/// XML-ридеру: у ридера на входе только байты дескриптора, каталога он не видит.
///
/// В `.cf` спутника нет — там картинка ИНЛАЙНИТСЯ прямо в тело формы
/// (`{4,3,{0},"",-1,-1,0,{{#base64:…}},0,""}`; base64 несёт РОВНО эти байты — сверено 17/17
/// mined-ячеек против `ssl.cf`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormPicture {
    /// Имя файла-спутника `<Tag>.<ext>`, где `<Tag>` — Designer-тег picture-свойства
    /// (`Picture` / `RowsPicture` / `ValuesPicture` / `HeaderPicture`; сверено 19/19 SSL:
    /// основа имени ВСЕГДА равна тегу). Тег ⇒ ключ свойства внутри контрола: у одного вида
    /// контрола два picture-свойства НИКОГДА не делят тег.
    pub file_name: String,
    /// Байты картинки как есть (§1.0 Blob).
    pub bytes: Vec<u8>,
}

impl FormPicture {
    /// Расширение файла-спутника (`zip`/`png`/`svg`) — оно же хвост канона `Ref("abs:<ext>")`.
    pub fn ext(&self) -> &str {
        self.file_name.rsplit('.').next().unwrap_or_default()
    }
}

/// Узел дерева формы (контрол).
///
/// Property-bag (`properties` и `ext_info`) ключуется тем же каноническим [`FieldId`], что
/// и у метаданных, — поэтому движок и стабильная сериализация одни и те же. `properties` —
/// ОБЩИЕ свойства контрола (видимость, размеры, dataPath…); `ext_info` — ТИП-СПЕЦИФИЧНЫЕ
/// свойства (`<extInfo xsi:type="form:<Type>ExtInfo">`/Designer-inline). Разделение
/// отражает структуру источника: оба формата держат extInfo-свойства в отдельном регионе.
///
/// Порядок детей и свойств ЗНАЧИМ (§3.2/§1.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormItem {
    /// Вид контрола (тип узла) — канонический дискриминатор.
    pub kind: FormControlKind,
    /// Имя элемента формы (стабильная идентичность узла внутри формы).
    pub name: String,
    /// Числовой `id` контрола (форм-локальный; платформенно присвоенный, общий обоим
    /// форматам). Часть идентичности узла, как `name`. Может быть отрицательным
    /// (служебные контролы вроде `autoCommandBar` несут `id == -1`).
    pub id: i64,
    /// Общие (не-тип-специфичные) свойства узла по каноническому `FieldId`, в порядке
    /// эмиссии спека (дефолты НЕ хранятся, §1.1). `Vec`, а не map — порядок структурен.
    #[serde(serialize_with = "crate::ir::serialize_semantic_properties")]
    pub properties: Vec<(FieldId, PropertyValue)>,
    /// ТИП-СПЕЦИФИЧНЫЕ свойства узла (`extInfo`) по каноническому `FieldId`, в порядке
    /// эмиссии extInfo-спека. Пуст для контролов без extInfo-региона.
    pub ext_info: Vec<(FieldId, PropertyValue)>,
    /// Обработчики событий узла (имя события → имя процедуры-обработчика).
    pub events: Vec<FormEvent>,
    /// Расширенная подсказка контрола (`extendedTooltip`/`ExtendedTooltip`), если есть.
    /// Несёт имя+id+ТЕЛО ([`DecoratorRef`]). ОБА формата несут СОДЕРЖАНИЕ тела (заголовок/
    /// `maxWidth`/`autoMax*`/`horizontalStretch`/события) — оно X-СРАВНИМО (после реконсиляции
    /// пер-форматных дефолтов). EDT дополнительно эмитит авто-КАРКАС (`type=Label`/extInfo-
    /// обёртка/`horizontalAlign`) — это форматно-асимметричная денормализация без Designer-
    /// аналога, нормализуемая ДО X.
    pub ext_tooltip: Option<DecoratorRef>,
    /// Контекстное меню контрола (`contextMenu`/`ContextMenu`), если есть. Та же модель, что
    /// [`Self::ext_tooltip`]: `autoFill` — пер-форматный дефолт (EDT эмитит `true` всегда,
    /// Designer опускает у авто-меню), X-сравним после реконсиляции.
    pub context_menu: Option<DecoratorRef>,
    /// Дочерние узлы (вложенные контролы/группы) в исходном порядке. Непуст только у
    /// контейнеров (FormGroup/Pages/Page/Popup/Table); листья несут пустой `Vec`.
    pub children: Vec<FormItem>,
    /// Табличные ДОБАВЛЕНИЯ (`searchStringAddition`/`viewStatusAddition`/
    /// `searchControlAddition`) — служебные под-контролы Таблицы. Каждый несёт `source`
    /// (dataPath), `autoMaxWidth` и decorator-стабы. Пусто у всех контролов, кроме `Table`.
    /// X-сравнимы (оба формата несут).
    #[serde(default)]
    pub additions: Vec<FormItem>,
    /// СОБСТВЕННАЯ командная панель контрола (Таблица несёт `<autoCommandBar>`/
    /// `<AutoCommandBar>`), если присутствует. `None` у контролов без панели.
    #[serde(default)]
    pub auto_command_bar: Option<AutoCommandBar>,
    /// Шрифт ПОДВАЛА поля (`footerFont` ⟺ `FooterFont`; ERP-witnessed 56×/33 файлов;
    /// cf-ячейка — {48}-тело [36], снята абляцией). `None` ⇒ отсутствует.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer_font: Option<FontRef>,
    /// СОБСТВЕННЫЕ исключённые стандартные команды контрола (Таблица: EDT
    /// `<excludedCommands>`-листы / Designer `<CommandSet><ExcludedCommand>`), в исходном
    /// порядке. Пусто у контролов без региона. X-сравнимы.
    #[serde(default)]
    pub excluded_commands: Vec<String>,
    /// Шрифт контрола (`<font>`/`<Font>`), если задан — композит-ссылка на стиль/системный
    /// шрифт плюс переопределения (Button/Decoration в корпусе). EDT
    /// `<font xsi:type="core:FontRef"><font>Style.X</font>…` ⟺ Designer
    /// `<Font ref="style:X" … kind="StyleItem"/>`. X-сравним (оба несут). `None` ⇒ шрифт не задан.
    /// У LabelField живёт в EDT-extInfo (`LabelFieldExtInfo.font`, метамодель backColor→font→
    /// useCopy) — Designer инлайн `<Font>` перед `<UseCopy>`; хранение общее (кинд хоста
    /// определяет регион на write).
    #[serde(default)]
    pub font: Option<FontRef>,
    /// Шрифт ЗАГОЛОВКА контрола (`<titleFont>`/`<TitleFont>`), если задан. Метамодель
    /// FormField/FormGroup: `titleTextColor` → `titleFont` → следующее поле тела; Designer
    /// эмитит `<TitleFont>` сразу после `<Title>` (witness LabelField/InputField/UsualGroup).
    /// X-сравним (оба несут). `None` ⇒ не задан.
    #[serde(default)]
    pub title_font: Option<FontRef>,
    /// Показ командной панели Таблицы — ТРИ-состояние `{true, false, auto}` (Designer
    /// кодирует ОДНИМ `<ShowCommandBar>значение`; EDT — ДВУМЯ полями: `<showCommandBar>true|
    /// false` для `true`/`false` и `<showCommandBarNeedDereferenced>true` для `auto`). Позиции
    /// в EDT РАЗНЫЕ: `showCommandBar` — после `commandBarLocation` (голова), а
    /// `showCommandBarNeedDereferenced` — в самом конце тела (после TAIL-полей). `None` ⇒ поле
    /// отсутствует (Table без явной панели). X-сравнимо (оба формата несут одно значение).
    #[serde(default)]
    pub show_command_bar: Option<String>,
    /// extInfo Таблицы-динамического-списка (`form:DynamicListTableExtInfo`), если Таблица —
    /// динамический список. EDT кодирует это ОБЁРТКОЙ `<extInfo xsi:type=
    /// "form:DynamicListTableExtInfo">` (последний ребёнок `<items form:Table>`); Designer —
    /// ИНЛАЙН-полями `<Table>` (`AutoRefresh`/`Period`/`TopLevelParent`/`ShowRoot`/…). Несёт
    /// набор канонических полей ([`DynamicListExt::fields`]) плюс СОБСТВЕННЫЕ обработчики
    /// (EDT `<extInfo><handlers>` — `OnGetDataAtServer`/…; Designer сливает их в единый
    /// табличный `<Events>` вместе с обычными событиями контрола) ⇒ X сливает
    /// [`DynamicListExt::events`] в [`Self::events`] (см. `normalize_form_for_x`). `None` ⇒
    /// Таблица НЕ является динамическим списком (обычная таблица данных). X-сравним (оба формата
    /// несут одинаковый канон-набор полей; расхождение значения ⇒ X=FALSE, не маскируется).
    #[serde(default)]
    pub dynamic_list_ext: Option<DynamicListExt>,
    /// АВТО-ТАБЛИЦА диаграммы (`GanttChartField`): вложенный контрол `Table`, который диаграмма
    /// несёт в СВОЁМ extInfo. EDT кодирует её как `<extInfo …GanttChartFieldExtInfo><autoTable>…`
    /// (тег `autoTable`, БЕЗ `xsi:type`), Designer — как прямого ребёнка `<Table>` элемента
    /// `<GanttChartField>`. Структурно — полноценная [`FormItem`] вида `Table` (тело/добавления/
    /// autoCommandBar/декораторы), поэтому читается/пишется ТЕМ ЖЕ Table-движком (X-сравнима как
    /// обычная таблица; нормализуется рекурсивно). `None` у всех контролов, кроме GanttChartField.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_table: Option<Box<FormItem>>,
    /// ИНЛАЙН-картинки (Blob-спутники) контрола — по одной на picture-свойство, чьё значение
    /// является СПУТНИКОМ, а не ссылкой на метаданные (канон `Ref("abs:<ext>")`). Пусто у
    /// подавляющего большинства контролов (19 спутников на весь SSL). См. [`FormPicture`].
    #[serde(default)]
    pub pictures: Vec<FormPicture>,
    /// Designer пометил `RowPictureDataPath` Таблицы СИГИЛОЙ `~` («поле не входит в состав полей
    /// динамического списка»): `<RowPictureDataPath>~Список.DefaultPicture</...>` — 5/312
    /// носителей SSL (динамические списки БЕЗ `<MainTable>`, чей произвольный запрос не выбирает
    /// стандартное поле `DefaultPicture`).
    ///
    /// EDT-экспорт ТЕХ ЖЕ форм пишет путь БЕЗ сигилы (во всём EDT-корпусе `~` не встречается ни
    /// разу), и edt→cf на всех пяти byte-exact ⇒ сигила в cf-тело НЕ кодируется: это чисто
    /// designer-локальная денормализация (как `conditional_appearance`/`list_settings`).
    /// ⇒ КАНОН = EDT-написание (путь без `~`), а сигила держится этим presence-точным флагом:
    /// `read_designer` её снимает и взводит флаг, `write_designer` возвращает, `normalize_form_for_x`
    /// зануляет. EDT-сторона его не несёт (false).
    ///
    /// Флаг, а не предикат: точное условие платформы — «лист не имеет `MainTable` И лист не
    /// ВЫБИРАЕТ это поле в своём запросе», и второй конъюнкт требует разбора текста запроса
    /// (3 носителя SSL — `НомерКартинки`/`ИндексКартинки` — именно так и опровергают более
    /// грубый предикат «нет MainTable»: сигилы у них нет, поле в запросе есть). §1.0: не
    /// угадываем — несём наблюдённый бит.
    #[serde(skip)]
    pub row_picture_path_unavailable: bool,
}

/// extInfo Таблицы-динамического-списка (`form:DynamicListTableExtInfo`).
///
/// # Асимметрия локусов (§1.6)
/// EDT держит поля в ОБЁРТКЕ `<extInfo>`; Designer — ИНЛАЙН в `<Table>`. Каноника (id/порядок/
/// кодек/пер-форматный дефолт) — в `spec/forms/controls/table` + `formats-xml/form/tables`
/// (`DYNAMIC_LIST_EXT`). Часть полей (`autoRefresh`/`choiceFoldersAndItems`/`restoreCurrentRow`/
/// `allowRootChoice`/`updateOnDataChange`) EDT НЕ эмитит НИКОГДА (Designer эмитит ВСЕГДА
/// константу корпуса) — реконсилируется [`crate::spec::common`]-политикой `Keep` (значение в
/// bag'е постоянно; расхождение ⇒ X=FALSE). `showRoot`/`allowGettingCurrentRowURL` EDT эмитит
/// лишь `true` (омит `false`), Designer эмитит ВСЕГДА — тоже `Keep`.
///
/// # Обработчики
/// EDT держит СОБСТВЕННЫЕ обработчики динамического списка (`OnGetDataAtServer`/…) в
/// `<extInfo><handlers>`; Designer сливает их в единый табличный `<Events>` (в платформенном
/// порядке — за обычными событиями контрола). Хранятся ОТДЕЛЬНО в [`Self::events`] (чтобы EDT-
/// запись легла byte-exact в обёртку), а для X сливаются в [`FormItem::events`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicListExt {
    /// Канонические поля extInfo по [`FieldId`], в КАНОНИЧЕСКОМ (= EDT-эмиссия) порядке —
    /// общий обоим форматам (X-сравним): `autoRefreshPeriod`/`period`/`topLevelParent`/
    /// `showRoot`/`allowGettingCurrentRowURL`/`userSettingsGroup`/`autoRefresh`/
    /// `choiceFoldersAndItems`/`restoreCurrentRow`/`allowRootChoice`/`updateOnDataChange`.
    /// Дефолты пер-форматных `Keep`-полей заполнены на read обоими ридерами ⇒ канон-bag равен.
    pub fields: Vec<(FieldId, PropertyValue)>,
    /// СОБСТВЕННЫЕ обработчики динамического списка (EDT `<extInfo><handlers>`), в исходном
    /// порядке. Designer сливает их в табличный `<Events>` ⇒ X сливает их в [`FormItem::events`].
    pub events: Vec<FormEvent>,
}

impl DynamicListExt {
    /// Значение канонического поля extInfo по id, если присутствует.
    pub fn get(&self, id: FieldId) -> Option<&PropertyValue> {
        self.fields.iter().find(|(k, _)| *k == id).map(|(_, v)| v)
    }
}

/// Привязка события контрола/формы к процедуре-обработчику.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormEvent {
    /// Каноническое имя события (`OnChange`, `OnClick`, `URLProcessing`, …).
    pub name: String,
    /// Имя процедуры-обработчика в модуле формы.
    pub handler: String,
}

impl FormItem {
    /// Создать пустой узел заданного вида с именем и id.
    pub fn new(kind: FormControlKind, name: impl Into<String>, id: i64) -> Self {
        FormItem {
            kind,
            name: name.into(),
            id,
            properties: Vec::new(),
            ext_info: Vec::new(),
            events: Vec::new(),
            footer_font: None,
            ext_tooltip: None,
            context_menu: None,
            children: Vec::new(),
            additions: Vec::new(),
            auto_command_bar: None,
            excluded_commands: Vec::new(),
            font: None,
            title_font: None,
            show_command_bar: None,
            dynamic_list_ext: None,
            auto_table: None,
            pictures: Vec::new(),
            row_picture_path_unavailable: false,
        }
    }

    /// Байты инлайн-картинки свойства с Designer-тегом `stem` (`Picture`/`RowsPicture`/
    /// `ValuesPicture`/`HeaderPicture`), если контрол её несёт. Ключ — ИМЯ ФАЙЛА-спутника,
    /// чья основа ВСЕГДА равна тегу (сверено 19/19 SSL), поэтому тег однозначно адресует
    /// свойство внутри контрола.
    pub fn picture(&self, stem: &str) -> Option<&FormPicture> {
        self.pictures
            .iter()
            .find(|p| p.file_name.split('.').next() == Some(stem))
    }

    /// Значение общего свойства по каноническому id, если присутствует.
    pub fn get(&self, id: FieldId) -> Option<&PropertyValue> {
        self.properties
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v)
    }

    /// Значение extInfo-свойства по каноническому id, если присутствует.
    pub fn get_ext(&self, id: FieldId) -> Option<&PropertyValue> {
        self.ext_info.iter().find(|(k, _)| *k == id).map(|(_, v)| v)
    }
}

/// Дефолт `AutoCommandBar::visible` при десериализации IR — `true` (панель видима, если
/// обратное не сказано ЯВНО; ровно тот же дефолт, что у обоих текстовых форматов).
fn acb_visible_default() -> bool {
    true
}

/// Автоматическая командная панель формы (`autoCommandBar`/`AutoCommandBar`).
///
/// Служебный фиксированный контрол с `id == -1`. Несёт имя, id, горизонтальное
/// выравнивание (`horizontalAlign`/`HorizontalAlign`, default `Auto`) и `autoFill`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoCommandBar {
    /// Имя панели (`ФормаКоманднаяПанель`).
    pub name: String,
    /// `id` панели (как правило `-1`).
    pub id: i64,
    /// Горизонтальное выравнивание (`Auto`/`Left`/`Right`/…); `None` = дефолт (`Auto`),
    /// не эмитится EDT.
    pub horizontal_align: Option<String>,
    /// Важность отображения (`displayImportance` ⟺ Designer-АТРИБУТ `DisplayImportance`
    /// на `<AutoCommandBar>`); `None` = дефолт, оба формата опускают. Symmetric (witness
    /// УправлениеПодключениемDSS.ЗаявлениеНаИзменениеРеквизитовПодключения: EDT
    /// name→id→displayImportance=Low→items; Designer `DisplayImportance="Low"`-атрибут).
    pub display_importance: Option<String>,
    /// Автозаполнение панели (`autoFill`/`Autofill`) — ДАННЫЕ с ПЕР-ФОРМАТНЫМИ дефолтами
    /// (cross-сверка 104/104 ACB корпуса): EDT-дефолт `false` (эмитит `<autoFill>true`),
    /// Designer-дефолт `true` (эмитит `<Autofill>false`). X-сравним по значению.
    pub auto_fill: bool,
    /// Дочерние контролы панели (как правило `Button`-команды). EDT — `<items>` под
    /// `<autoCommandBar>`; Designer — `<ChildItems>` под `<AutoCommandBar>`. Пусто ⇒
    /// служебная пустая панель (как у пилота).
    pub items: Vec<FormItem>,
    /// Видимость панели (`visible`/`Visible`) — ДАННЫЕ, дефолт `true` (оба формата опускают
    /// значение по умолчанию). Волны 1-25 держали её КОНСТАНТОЙ `1` в cf (ни SSL, ни ERP не
    /// несут скрытой ACB), т.е. свойство молча терялось; витнесс — `integration_subsystem`,
    /// `CommonForm.инт_Подписчики` ACB `ПодписчикиКоманднаяПанель` (id 7) c
    /// `<Visible>false</Visible>`: платформенная абляция (собрать конфиг ДВАЖДЫ, со свойством и
    /// без) даёт РОВНО ОДИН отличающийся байт — первую ячейку хвоста `{22,…}`-записи, ту самую,
    /// где у обычной группы стоит её `visible`.
    #[serde(default = "acb_visible_default")]
    pub visible: bool,
    /// Designer сериализовал ФОРМ-уровневую панель с ЯВНЫМ именем `FormCommandBar`
    /// (нерегулярность: 3/876 SSL-форм — ТранспортСообщенийОбменаESB1C и др.; остальные несут
    /// ПУСТОЕ имя, канонизуемое к `FormCommandBar`). Presence-точный флаг Designer-роундтрипа;
    /// EDT-сторона его не несёт (false).
    #[serde(default)]
    pub designer_named: bool,
}
