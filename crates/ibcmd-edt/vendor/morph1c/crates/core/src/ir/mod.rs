//! Семантический IR (`L1`, ARCHITECTURE.md §1.2): `Configuration → MetadataObject*
//! → (properties, children, modules, forms, templates)`. Это «внутренний формат».
//!
//! Инварианты (§1.1, §1.3, §1.6):
//! * **Формат-нейтральность** — ничего форматного (ни slot-номеров `.cf`, ни имён
//!   XML-тегов). Проекция живёт в спеке формата, не здесь.
//! * **Канонический id** — каждое свойство ключуется [`FieldId`], заданным ОДИН раз
//!   в `spec` (§1.6); оба формата дают равный IR.
//! * **Порядок исходника сохранён** — `properties`/`children` — упорядоченные `Vec`.
//! * **Мешок свойств = то, ЧТО БЫЛО В ИСТОЧНИКЕ** — умолчания в записи НЕ дублируются
//!   (их миллионы, IR целой ERP — порядка 10 ГБ), но и не теряются: они РАЗРЕШИМЫ из
//!   IR через [`crate::resolve`] (мешок + спек + [`Configuration::source_version`]).
//!   Присутствие поля в мешке означает «задано явно», отсутствие — «действует
//!   умолчание версии»; writer восстанавливает умолчание по конвенции своего формата.
//!   Прежняя формулировка «дефолты не хранятся» (§1.1) уточнена именно так: не
//!   хранятся КОПИИ, но ответ «каково значение» из IR получить можно всегда.
//! * **Версия источника — часть IR** ([`Configuration::source_version`]). Прежнее
//!   правило §1.6 «версия — свойство файла, в семантический IR не течёт» ОТМЕНЕНО:
//!   без версии мешок не отличает «не было в источнике» от «есть, но равно умолчанию»,
//!   и потребителям приходилось протаскивать версию мимо IR отдельным параметром.
//! * **Одна сущность — один тип** — объект моделируется РОВНО [`MetadataObject`],
//!   без subset+union (§1.1).

pub mod form;
pub mod value;
pub mod source_extensions;

use serde::{Deserialize, Serialize};

pub use form::{
    AdditionalColumns, AutoCommandBar, ContextMenuBody, ControlKind, DcsAvailableValue,
    DcsCalculatedField, DcsCorValue, DcsField, DcsItem, DcsListSettings, DcsOrderExpression,
    DcsParamValue, DcsParameter, DcsPresentation,
    DcsRightValue, DcsSelectionField, DcsSettingsGroup, DcsSettingsParameterValue, DcsUseRestriction,
    DecoratorBody, DecoratorRef,
    DocumentFormInfo, DynamicListAttrExt, DynamicListExt, FontRef, FormBody, FormCiItem,
    FormCommand, FormControlKind, FormDataAttribute, FormEvent, FormItem, FormParameter,
    FormRootExtInfo, NamedFormBody, ReportFormInfo, TooltipBody, DOCUMENT_AUTO_TIME_DEFAULT,
    DOCUMENT_USE_POSTING_MODE_DEFAULT, FOLDERS_AND_ITEMS_DEFAULT, REPORT_FORM_AUTO,
};
// Платформенный ПОРЯДОК/идентичность форм-событий (`ir/form/event_order.rs`): общий обоим
// формат-слоям — cf-реестр событий И Designer-`<Events>` сортируются ОДНИМ guid-ключом.
pub use form::{
    form_event_guid, merge_form_events, merge_table_events, table_event_guid, TABLE_EVENT_GUIDS,
};
pub use value::{
    BlobRef, ColorStyle, DateFractions, FontFace, FontFlags, FontStyle, Lang, PropertyValue,
    StyleValueSpec,
    Token, TypeQualifier, TypeRef, TypeSpec, ValueKind, ValueScalarKind, ValueSpec,
};

// Adapter-local semantic serialization: projection-specific property enumeration
// is not identity. Sort borrowed references only; never copy retained bodies or
// change the private IR's source order used by typed writers.
fn serialize_semantic_properties<S: serde::Serializer>(
    properties: &[(FieldId, PropertyValue)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut ordered = properties.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(id, _)| *id);
    ordered.serialize(serializer)
}

/// Канонический, формат-нейтральный идентификатор свойства.
///
/// Это КЛЮЧ инварианта §1.6: id свойства задаётся ОДИН раз в `spec` (через
/// [`crate::spec::FieldSpec`]) и одинаков во всех форматах. Формат проецирует его в
/// своё представление (имя тега / slot / столбец), но в IR живёт только этот
/// канонический id — поэтому `read_edt(o)` и `read_designer(o)` ключуют одно
/// свойство одинаково.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldId(pub u32);

impl FieldId {
    /// Числовое значение id.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FieldId({})", self.0)
    }
}

/// UUID объекта/свойства как формат-нейтральная 16-байтовая идентичность.
///
/// Хранится сырыми байтами (а не строкой), чтобы регистр/разделители текстового
/// представления формата не влияли на равенство IR (§1.6): cross-format-гейт (§3.5)
/// связывает объекты по `uuid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Uuid(pub [u8; 16]);

/// Вид объекта метаданных — канонический код (`Catalog`, `Document`,
/// `InformationRegister`, …), формат-нейтральный.
///
/// Строковый код (а не закрытый enum), чтобы новые виды добавлялись модулем-на-вид
/// в `spec/metadata` без правки ядра IR (§5), оставаясь одним значением во всех
/// форматах.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectKind(pub String);

impl ObjectKind {
    /// Построить вид из кода.
    pub fn new(s: impl Into<String>) -> Self {
        ObjectKind(s.into())
    }
    /// Код вида как `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Модуль 1С, прикреплённый к объекту (модуль объекта, менеджера, …).
///
/// F2a-скелет: имя «слота» модуля + тело. Тело — либо обычный текстовый исходник (99% случаев),
/// либо ЗАЩИЩЁННЫЙ (protected) бинарный образ (штатный механизм защиты 1С). Семантика разбора
/// кода вне F2a.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// Канонический код слота модуля (`ObjectModule`, `ManagerModule`, …).
    pub slot: String,
    /// Тело модуля (текст или защищённый бинарь).
    pub body: ModuleBody,
}

/// Тело модуля: обычный текст или защищённый (protected) бинарный образ.
///
/// §1.0: защищённый образ — ОПАКОВЫЙ. На диске он лежит самодостаточным v8-контейнером
/// (сигнатура `ff ff ff 7f …`, внутри ресурсы `image` + `info`=`{3,2,0,"",0}`); мы его НЕ
/// парсим, НЕ нормализуем и НЕ трогаем EOL — носим ДОСЛОВНО. Образ ДИАЛЕКТ-НЕЗАВИСИМ: edt
/// `<Слот>.bsl` и designer `<Name>/Ext/<Слот>.bin` для одного модуля БАЙТ-ИДЕНТИЧНЫ
/// (сверено md5 на 10 защищённых модулях ERP), т.е. единый канон.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleBody {
    /// Обычный текстовый исходник (хранится как есть; brace/EOL — забота формата).
    Text(String),
    /// Защищённый образ 1С — сырые байты самодостаточного v8-контейнера, ДОСЛОВНО.
    Binary(Vec<u8>),
}

impl Module {
    /// Построить текстовый модуль.
    pub fn text(slot: impl Into<String>, source: impl Into<String>) -> Self {
        Module {
            slot: slot.into(),
            body: ModuleBody::Text(source.into()),
        }
    }

    /// Построить защищённый (бинарный) модуль из сырых байт образа.
    pub fn binary(slot: impl Into<String>, bytes: Vec<u8>) -> Self {
        Module {
            slot: slot.into(),
            body: ModuleBody::Binary(bytes),
        }
    }

    /// Текст модуля, если он текстовый; `None` для защищённого бинаря.
    pub fn as_text(&self) -> Option<&str> {
        match &self.body {
            ModuleBody::Text(s) => Some(s),
            ModuleBody::Binary(_) => None,
        }
    }
}

/// Двоичное тело картинки вида `CommonPicture` — сырые байты изображения + имя файла.
///
/// EDT хранит его сиблингом `<obj-dir>/Picture.<ext>`; Designer — как
/// `<dir>/<Name>/Ext/Picture/<file>` + XML-обёртку `<dir>/<Name>/Ext/Picture.xml`
/// (`<xr:Abs><file></xr:Abs>`, `LoadTransparent=false` — константа обёртки). Сырые байты
/// изображения идентичны между форматами (§1.6). Носится ОТДЕЛЬНО от `properties` (спутник, не
/// спек-свойство): whole-config read подгружает его (`pipeline::picture_read`), whole-config write
/// эмитит в per-format-путь.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PictureBody {
    /// Имя файла изображения (напр. `Picture.png`) — общее для EDT-файла, Designer
    /// `Ext/Picture/<file>` и ссылки `<xr:Abs>` в обёртке.
    pub file_name: String,
    /// Сырые байты изображения (идентичны между форматами).
    pub bytes: Vec<u8>,
}

/// WSDL-описание WS-ссылки (`WSReference`) — канонический (BOM-снятый) текст WSDL плюс
/// приложенные XSD-схемы.
///
/// Раскладка сайдкаров (RE `.fixtures/ERP` — 2/2 WSReference):
/// * **Designer**: `WSReferences/<Name>/Ext/WSDefinition.xml` (с BOM) + `Ext/<N>.xsd`
///   (числовые имена, без BOM);
/// * **EDT**: `WSReferences/<Name>/WsDefinitions.wsdl` (без BOM) + сиблинги `<N>.xsd`.
///
/// Байты идентичны между форматами после снятия BOM у WSDL (§1.6, сверено ERP 2/2). Носится
/// ОТДЕЛЬНО от `properties` (спутник, не спек-свойство): whole-config read подгружает его
/// (`pipeline::ws_definition_read`), а `--to cf` эмитит тело `<uuid>.0` — ВЛОЖЕННЫЙ 32-битный
/// v8-контейнер с raw-элементами `0.wsdl` + `<N>.xsd` (RE erp.cf 2/2 —
/// `formats_cf::ws_reference_body`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsDefinition {
    /// Канонический (BOM-снятый) текст WSDL-описания.
    pub wsdl: Vec<u8>,
    /// Приложенные XSD-схемы `(имя файла `<N>.xsd`, байты verbatim)` в числовом порядке `N`
    /// (порядок = FAT-порядок cf-тела и порядок числовых имён на диске).
    pub xsds: Vec<(String, Vec<u8>)>,
}

/// Картинка КОРНЯ конфигурации (`Splash` / `MainSectionPicture`) — слот + расширение +
/// сырые байты изображения (config-level Ext, RE coverage/s15_subordinate).
///
/// Раскладка расходится с [`PictureBody`] (CommonPicture): у корня картинка живёт в
/// config-level `Ext` и имя файла ПЕР-ФОРМАТНОЕ:
/// * **EDT**: `src/Configuration/<Слот>.<ext>` (сиблинг `Configuration.mdo`, имя = слот);
/// * **Designer**: `Ext/<Слот>.xml` (ExtPicture-обёртка, `<xr:Abs>Picture.<ext></xr:Abs>`,
///   `LoadTransparent=false`) + сырое `Ext/<Слот>/Picture.<ext>`.
///
/// Сырые байты идентичны между форматами (§1.6, сверено s15) → канон несёт слот +
/// расширение + байты; пер-форматные имена файлов регенерируются. Дескриптор ссылку НЕ несёт
/// (EDT `<splash/>`/`<mainSectionPicture/>` — всегда-пустые узлы, Designer — ничего):
/// присутствие картинки гейтится ФАЙЛОМ на диске. Спутник, не спек-свойство (как
/// [`Module`]/[`PictureBody`]): whole-config read подгружает (`pipeline::ext_read`),
/// whole-config write эмитит в пер-форматную раскладку.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigPicture {
    /// Слот картинки корня (`"Splash"` / `"MainSectionPicture"`).
    pub slot: String,
    /// Расширение файла изображения без точки (`"png"`).
    pub ext: String,
    /// Сырые байты изображения (идентичны между форматами).
    pub bytes: Vec<u8>,
}

/// БИНАРНЫЙ конфиг-сайдкар КОРНЯ конфигурации (`ParentConfigurations` /
/// `MobileClientSignature`) — слот + СЫРЫЕ байты файла ВЕРБАТИМ (config-level `Ext`,
/// как [`ConfigPicture`], но БЕЗ какой-либо перекодировки: это не модуль — BOM/CRLF
/// не трогаются, байты несутся дословно).
///
/// Витнессы (byte-identical, сверено sha256):
/// * **`ParentConfigurations`**: ERP designer `Ext/ParentConfigurations.bin` (16 Б) ==
///   ERP edt `src/Configuration/ParentConfigurations.bin` == erp.cf `<host>.4`;
///   SSL designer == SSL edt (619 225 Б) == ssl.cf `<host>.4`.
/// * **`MobileClientSignature`**: ERP designer `Ext/MobileClientSignature.bin`
///   (560 886 Б, brace `{2,"MIIBtjCC…"}`) == ERP edt `src/Configuration/
///   MobileClientSign.bin` (⚠️ EDT-имя ДРУГОЕ — `MobileClientSign.bin`) ==
///   erp.cf `<host>.10` (hex-слот 16). У SSL файла нет ни в одном диалекте и в
///   ssl.cf нет `.10` — отсутствие честно (гейтится файлом на диске).
///
/// Спутник, не спек-свойство (как [`Module`]/[`ConfigPicture`]): whole-config read
/// подгружает (`pipeline::ext_read`), whole-config write эмитит в пер-форматное имя,
/// `--to cf` эмитит verbatim-тело `<host>.N` (`formats_cf::assemble` CONFIG_BLOB_SLOTS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigBlob {
    /// Слот сайдкара (`"ParentConfigurations"` / `"MobileClientSignature"`).
    pub slot: String,
    /// Сырые байты файла — ВЕРБАТИМ (идентичны между форматами, вкл. BOM).
    pub bytes: Vec<u8>,
}

/// ОДНА запись приоритета обмена автономной конфигурации ([`StandaloneContent`]):
/// полное дот-имя объекта метаданных + witnessed-значение приоритета.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandalonePriority {
    /// Полное дот-имя объекта (`Kind.Name`, напр. `Constant.ПринятоЗаписей`).
    pub metadata: String,
    /// Значение приоритета — enum-строка диалектов (`LocalServer` — единственное
    /// witnessed-значение корпуса; cf-код резолвится ТОЛЬКО по witnessed-карте, §1.0).
    pub priority: String,
}

/// СОСТАВ АВТОНОМНОЙ (мобильной) КОНФИГУРАЦИИ — структурный конфиг-сайдкар КОРНЯ
/// (designer `Ext/StandaloneConfigurationContent.bin`, edt
/// `src/Configuration/MobileApplicationContent.scc`). В отличие от [`ConfigBlob`]
/// НЕ verbatim: оба диалекта — XML РАЗНОЙ структуры (designer — элементный
/// `<StandaloneContent xmlns="…xcf/extrnprops" version="2.20">` с `<UsedItem><Metadata>`;
/// edt — атрибутный `<scc:StandaloneContent xmlns:scc="http://g5.1c.ru/v8/dt/scc">` с
/// `<usedItem metadata="…"/>`), несущей ОДИН и тот же канон (сверено ERP: списки/порядок/
/// настройки байт-в-байт эквивалентны, §1.6). cf-тело `<host>.f` резолвит дот-имена в
/// object-uuid'ы при сборке (`formats_cf::assemble` — реестр обязателен, §1.0).
///
/// Витнесс ERP (единственный носитель корпуса; SSL/coverage файла НЕ имеют — отсутствие
/// честно): 1692 used + 596 unused (unused — В Т.Ч. дочерние пути `Kind.Obj.Attribute.X`/
/// `…Form.X`/`…Template.X`/`…TabularSection.TS.Attribute.X`/`Subsystem.P.Subsystem.C`) +
/// 3 priority (`LocalServer`) + настройки обмена (true/300/1000/0).
///
/// ⚠️ ПОРЯДОК списков: канон IR = документный порядок XML-диалектов. Эталонное cf-тело
/// `erp.cf <host>.f` несёт ТЕ ЖЕ множества в ДРУГОМ порядке (внутренний порядок платформы,
/// в дампе НЕ сохранён — из исходников невоспроизводим); витнесс-факт: XML-порядок дампа =
/// стабильная сортировка cf-порядка по ПЕРВОМУ байту uuid (проверено 1692/596/3), т.е.
/// документный порядок — НЕПОДВИЖНАЯ ТОЧКА пере-дампа платформы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandaloneContent {
    /// Используемые объекты — полные дот-имена, документный порядок XML.
    pub used: Vec<String>,
    /// НЕиспользуемые элементы — полные дот-имена (вкл. дочерние пути), документный порядок.
    pub unused: Vec<String>,
    /// Приоритеты обмена ([`StandalonePriority`]), документный порядок.
    pub priority: Vec<StandalonePriority>,
    /// `ExchangeOnChangeData` (designer) / `exchangeOnChangeData` (edt) — witnessed `true`.
    pub exchange_on_change_data: bool,
    /// `ExchangePeriod` / `exchangePeriod`, секунды — witnessed `300`.
    pub exchange_period: u64,
    /// `TransactionCount` / `transactionCount` — witnessed `1000`.
    pub transaction_count: u64,
    /// `InactiveNodesCleanupTimeout` (designer, ВНУТРИ `DataExchangeSettings`) — witnessed
    /// `0`; у EDT-диалекта поля НЕТ вовсе (witnessed-значение 0 = дефолт; ненулевое
    /// значение в edt-записи — типизированный §1.0-отказ, кодировка не witnessed).
    pub inactive_nodes_cleanup_timeout: u64,
}

/// ОДНА страница справки объекта (`Help/<lang>.html`) — язык + канонический HTML-текст.
///
/// Канон: BOM-снятый, EOL-нормализованный к `\n` текст (§1.6: edt==designer). Раскладка и
/// пер-форматное кодирование — RE coverage/s15_subordinate (15 объектов × `ru.html`):
/// * **EDT**: `<obj-dir>/Help/<lang>.html` — БЕЗ BOM, CRLF;
/// * **Designer**: `<dir>/<Name>/Ext/Help/<lang>.html` — С BOM, LF, ПЛЮС дескриптор
///   `Ext/Help.xml` (`<Page><lang></Page>` на страницу), регенерируемый из списка страниц.
///
/// Спутник, не спек-свойство (как [`Module`]/[`PictureBody`]): whole-config read подгружает
/// его (`pipeline::help_read`), whole-config write эмитит в пер-форматную раскладку.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelpPage {
    /// Код языка страницы (`ru`, `en`, …) — имя файла `<lang>.html` == Designer `<Page>`.
    pub lang: String,
    /// Канонический HTML-текст страницы (без BOM, EOL = `\n`).
    pub body: String,
}

/// ОДИН вспомогательный файл-ресурс справки из каталога `_files/` рядом со страницами
/// (`Help/_files/…` в EDT / `Ext/Help/_files/…` в Designer) — картинки и пр., на которые
/// ссылается HTML страниц.
///
/// §1.0: ОПАКОВЫЙ байт-вербатимный транспорт (witnessed: SSL `ТранспортСообщенийОбменаESB1C`,
/// один `.png`, байт-идентичный между диалектами) — содержимое НЕ интерпретируется, никакого
/// пере-кодирования. Designer-дескриптор `Ext/Help.xml` ресурсы НЕ объявляет (только `<Page>`).
/// Спутник, не спек-свойство (как [`HelpPage`]): читает/пишет `pipeline::help_read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelpResource {
    /// Путь файла ОТНОСИТЕЛЬНО каталога `_files/`, `/`-разделённый (канон обоих диалектов).
    pub rel_path: String,
    /// Сырые байты файла — вербатим (идентичны между форматами).
    pub bytes: Vec<u8>,
}

/// РАСПИСАНИЕ регламентного задания (`ScheduledJob`) — сайдкар-спутник дескриптора
/// (как [`HelpPage`]/[`PictureBody`]), а НЕ спек-свойство: whole-config read подгружает его
/// (`pipeline::schedule_read`), whole-config write эмитит в пер-форматную раскладку, а
/// cf-сборщик — телом `<uuid>.0` (`formats_cf::schedule_body`).
///
/// # Раскладка (RE SSL: 45 из 50 ScheduledJob'ов несут сайдкар, оба диалекта)
/// * **EDT** — `<obj-dir>/Schedule.schedule`: корень `<schedule:Schedule>` (ns
///   `http://g5.1c.ru/v8/dt/schedule`), поля — АТРИБУТЫ в фиксированном порядке схемы, ДЕФОЛТЫ
///   ОПУЩЕНЫ; дни недели/месяцы — ПОВТОРЯЮЩИЕСЯ элементы с ИМЕНАМИ (`<weekDays>Mon</weekDays>`,
///   `<months>Jan</months>`); вложенные — `<dailySchedules …/>`.
/// * **Designer** — `<dir>/<Name>/Ext/Schedule.xml`: корень `<JobSchedule>` (ns extrnprops)
///   → `<Schedule …>` со ВСЕМИ 12 атрибутами ВСЕГДА; дни/месяцы — ОДИН элемент со
///   СПИСКОМ ЧИСЕЛ (`<ent:WeekDays>1 2 3 4 5 6 7</ent:WeekDays>`); вложенные —
///   `<ent:DetailedDailySchedules …>`.
///
/// Канон IR — Designer-полнота (все поля явные, дни/месяцы — числа 1..7 / 1..12), потому что
/// именно её требует cf-тело; EDT-ридер восстанавливает опущенные дефолты, EDT-райтер их снова
/// опускает (§1.6: оба диалекта дают РАВНЫЙ [`Schedule`]).
///
/// РЕКУРСИВНО: `daily_schedules` — «детальные» суточные расписания (witnessed: SSL
/// `ОбновлениеАгрегатов`, 2 вложенных).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Schedule {
    /// `BeginDate` / `beginDate` — дата начала (канон `YYYY-MM-DD`, дефолт `0001-01-01`).
    pub begin_date: ScheduleDate,
    /// `EndDate` / `endDate`.
    pub end_date: ScheduleDate,
    /// `BeginTime` / `beginTime` — время (канон `HH:MM:SS`, дефолт `00:00:00`).
    pub begin_time: ScheduleTime,
    /// `EndTime` / `endTime`.
    pub end_time: ScheduleTime,
    /// `CompletionTime` / `completionTime`.
    pub completion_time: ScheduleTime,
    /// `CompletionInterval` / `completionInterval` — секунды. Дефолт 0.
    pub completion_interval: i64,
    /// `RepeatPeriodInDay` / `repeatPeriodInDay` — секунды. Дефолт 0.
    pub repeat_period_in_day: i64,
    /// `RepeatPause` / `repeatPause` — секунды. Дефолт 0.
    pub repeat_pause: i64,
    /// `WeekDayInMonth` / `weekDayInMonth`. Дефолт 0.
    pub week_day_in_month: i64,
    /// `DayInMonth` / `dayInMonth`. Дефолт 0.
    pub day_in_month: i64,
    /// `WeeksPeriod` / `weeksPeriod`. `platform_default` = 1 (дефолт ВЛОЖЕННОГО узла), но
    /// EDT-КОРЕНЬ трактует омиссию как 0 (witnessed ERP: 11 файлов без `weeksPeriod` ↔
    /// Designer `WeeksPeriod="0"`; 247 с `weeksPeriod="1"`) — правило «числовой атрибут корня
    /// пишется ⇔ значение ≠ 0», а НЕ «всегда» (SSL 45/45 с `weeksPeriod="1"` это правило
    /// не различал — класс `const-modeled settable field`).
    pub weeks_period: i64,
    /// `DaysRepeatPeriod` / `daysRepeatPeriod`. Дефолт 0.
    pub days_repeat_period: i64,
    /// Дни недели, 1..7 (Пн..Вс) — порядок как в источнике. Пусто — «все дни» (вложенные несут []).
    pub week_days: Vec<i64>,
    /// Месяцы, 1..12 — порядок как в источнике.
    pub months: Vec<i64>,
    /// Вложенные детальные суточные расписания (рекурсия).
    pub daily_schedules: Vec<Schedule>,
}

/// Дата расписания в КАНОНЕ `YYYY-MM-DD` (общий текст обоих диалектов; cf кодирует её как
/// `YYYYMMDDhhmmss`). Тонкая обёртка над `String` — чтобы дата и время не путались местами.
pub type ScheduleDate = String;
/// Время расписания в КАНОНЕ `HH:MM:SS` (общий текст обоих диалектов).
pub type ScheduleTime = String;

impl Schedule {
    /// Расписание с ДЕФОЛТАМИ платформы (то, что EDT опускает): даты `0001-01-01`, времена
    /// `00:00:00`, счётчики 0, `weeks_period` = 1.
    pub fn platform_default() -> Self {
        Schedule {
            begin_date: SCHEDULE_ZERO_DATE.to_string(),
            end_date: SCHEDULE_ZERO_DATE.to_string(),
            begin_time: SCHEDULE_ZERO_TIME.to_string(),
            end_time: SCHEDULE_ZERO_TIME.to_string(),
            completion_time: SCHEDULE_ZERO_TIME.to_string(),
            completion_interval: 0,
            repeat_period_in_day: 0,
            repeat_pause: 0,
            week_day_in_month: 0,
            day_in_month: 0,
            weeks_period: 1,
            days_repeat_period: 0,
            week_days: Vec::new(),
            months: Vec::new(),
            daily_schedules: Vec::new(),
        }
    }
}

/// Дефолтная («нулевая») дата расписания — witnessed 45/45 в обоих диалектах.
pub const SCHEDULE_ZERO_DATE: &str = "0001-01-01";
/// Дефолтное («нулевое») время расписания.
pub const SCHEDULE_ZERO_TIME: &str = "00:00:00";

/// ОДНА запись СОСТАВА СТИЛЯ (`Style`) — элемент сайдкара-спутника `Style.style` (EDT) /
/// `Ext/Style.xml` (Designer), читаемого `pipeline::style_read`; cf-сборщик эмитит список
/// телом `<uuid>.0` (`formats_cf::style_body`). Спутник, не спек-свойство (как [`Schedule`]).
///
/// Витнесс — ЕДИНСТВЕННЫЙ носитель корпусов: ERP `Styles/Основной` (231 запись, оба диалекта,
/// ОДИН И ТОТ ЖЕ doc-порядок — §1.6: оба дают РАВНЫЙ канон). Запись = имя элемента стиля +
/// значение; имя: СТАНДАРТНЫЙ платформенный элемент (`FormBackColor`, `TextFont`,
/// `ControlBorder`, …) ЛИБО конфиг-элемент `StyleItem.<Имя>` (префикс — канон обоих
/// диалектов). Порядок записей структурен (== doc-порядку источника).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleRecord {
    /// Имя элемента стиля: `FormBackColor`/…/`ControlBorder` (стандартный) либо
    /// `StyleItem.<Имя>` (конфиг-StyleItem; резолв в uuid — забота cf-эмиттера).
    pub name: String,
    /// Значение элемента (вид значения == виду элемента: Color/Font/Border).
    pub value: StyleRecordValue,
}

/// Значение записи состава стиля. Color/Font реюзают канон стиль-значений
/// ([`value::ColorStyle`]/[`value::FontStyle`] — те же формы, что у StyleItem, §1.6);
/// Border в составе стиля засвидетельствован ТОЛЬКО ССЫЛКОЙ (`BorderRef` — ERP
/// `ControlBorder` → `Style.ControlBorder`; абсолютный `BorderDef` в составе стиля не
/// witnessed → кодеки на нём честно отказывают, §1.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StyleRecordValue {
    /// Цвет (`<Color>` Designer / `style:ColorStyleItem` EDT).
    Color(value::ColorStyle),
    /// Шрифт (`<Font …/>` Designer / `style:FontStyleItem` EDT).
    Font(value::FontStyle),
    /// ССЫЛКА на рамку-стиль (`<Border ref="style:X"/>` Designer / `core:BorderRef` EDT);
    /// канон — `Style.<Имя>` (witnessed ERP: только `Style.ControlBorder`).
    BorderRef(String),
}

/// Платформенно-вычисляемый тип, сгенерированный платформой для объекта (§1.6,
/// §3.5). 1С присваивает каждому ссылочно-порождающему объекту (DefinedType,
/// Catalog, …) стабильные внутренние **type-id GUID**'ы, которые ПРИСУТСТВУЮТ в
/// исходниках обоих XML-форматов:
/// * EDT — `<producedTypes><containerType typeId=… valueTypeId=…/></producedTypes>`;
/// * Designer — `<InternalInfo><xr:GeneratedType name=… category=…><xr:TypeId/>
///   <xr:ValueId/></xr:GeneratedType></InternalInfo>`.
///
/// GUID'ы — ОПАКОВЫЕ платформенные присвоения (не выводимы из uuid+Type детерминированно),
/// поэтому ЗАХВАТЫВАЮТСЯ, чтобы воспроизвести байт-в-байт (§3.2). Подтверждено сканом
/// SSL (72/72 DefinedType): `(type_id, value_id)` ПОБАЙТОВО РАВНЫ между edt и designer
/// для одного объекта — значит это КАНОНИЧЕСКОЕ значение IR (X by construction, §1.6),
/// а НЕ источниковая проекция. Designer-атрибуты `name`/`category` НЕ хранятся: они
/// детерминированно регенерируются из вида+имени объекта (`<Kind>.<Name>` / `<Kind>`).
///
/// Это СТРУКТУРНЫЙ XML (а не опаковый бинарь), поэтому он типизирован в IR, а НЕ скрыт
/// в [`PropertyValue::Blob`] (ARCHITECTURE.md §1.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedType {
    /// Категория сгенерированного типа — формат-нейтральный КАНОН (`"Container"` для
    /// одиночного producedType DefinedType; `"Ref"`/`"List"`/`"Manager"` для Enum и
    /// др. ссылочных видов). Канонизирует РАСХОЖДЕНИЕ ПОРЯДКА между форматами (EDT
    /// эмитит ref/list/manager, Designer — Ref/Manager/List): IR хранит ОДИН
    /// канонический порядок, каждый формат восстанавливает свой через category↔тег/имя
    /// маппинг каркаса коннектора (§1.6). Designer-`name`/`category` и EDT-имя-тега —
    /// ДЕТЕРМИНИРОВАННО выводимы из (вид, category), поэтому хранится лишь category.
    pub category: String,
    /// `typeId` (EDT) / `xr:TypeId` (Designer) — GUID типа-контейнера. Байты, не
    /// строка: регистр/разделители текстовой формы не влияют на равенство (как [`Uuid`]).
    pub type_id: Uuid,
    /// `valueTypeId` (EDT) / `xr:ValueId` (Designer) — GUID типа-значения.
    pub value_id: Uuid,
}

/// Блок платформенно-вычисляемой служебной информации объекта (§1.6). Сейчас несёт
/// только список [`GeneratedType`] (`producedTypes`/`InternalInfo>GeneratedType`).
/// Каркас коннектора (НЕ спек-движок) читает/пишет его в фиксированной структурной
/// позиции (до `<name>` в EDT, до `<Properties>` в Designer) — как идентичность
/// `name`/`uuid`, а не как спек-свойство.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalInfo {
    /// Сгенерированные платформой типы в исходном порядке (в SSL — ровно один на
    /// DefinedType; `Vec` заложен на случай нескольких записей у других видов).
    pub generated_types: Vec<GeneratedType>,
}

/// Видимость ОДНОЙ команды в командном интерфейсе подсистемы (`Subsystem`) — один
/// `<visibilityFragments>` (EDT) / `<Command>` (Designer) фрагмент. Носит ПОЛНОЕ имя
/// команды (`Catalog.X.StandardCommand.OpenList`) как есть; разбор на объект-путь +
/// имя стандартной команды — забота формата на записи (§1.6, cf `cmi_body`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandVisibility {
    /// Полное имя команды: `<Kind>.<Name>.StandardCommand.<Cmd>` (канон, edt==designer).
    pub command: String,
    /// Общая видимость команды (`<xr:Common>`/`<common>`): `true` — видима, `false` —
    /// скрыта. EDT `<visible/>` (пусто) == `false`; `<visible><common>true</common>` == `true`.
    pub common_visible: bool,
    /// ПО-РОЛЕВЫЕ переопределения видимости в ИСХОДНОМ порядке (witnessed ERP: 102 записи,
    /// 97 `true` / 5 `false`). Designer — `<xr:Value name="Role.X">BOOL</xr:Value>` ПОСЛЕ
    /// `<xr:Common>`; EDT — `<for><value>true</value><role>Role.X</role></for>` (`<value>`
    /// опущен == `false`). Пусто ⇒ переопределений нет (весь SSL-корпус).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_values: Vec<RoleVisibility>,
}

/// Переопределение видимости команды для ОДНОЙ роли (см. [`CommandVisibility::role_values`]).
/// cf-тело `<uuid>.1` кодирует запись как `roleUuid, {"B", <0|1>}` внутри visRecord
/// (witnessed erp.cf: `Subsystem.CRMИМаркетинг` ↔ его Designer/EDT сайдкары).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleVisibility {
    /// Полное имя роли: `Role.<Name>` (канон, edt==designer; cf резолвит в uuid объекта).
    pub role: String,
    /// Значение видимости для роли (`true`/`false` — оба witnessed).
    pub visible: bool,
}

/// Фрагмент региона `commandsPlacement`/`commandsOrder` — ОДНА группа команд. В EDT это
/// прямой `<placementFragments>`/`<orderFragments>` (`<group>` + список `<commands>`); в
/// Designer это ТРАНСПОНИРОВАННЫЙ per-command вид (`<Command name><CommandGroup>G` — один
/// `<Command>` на каждую команду фрагмента). Канон хранит EDT-сгруппированную форму; Designer
/// ридер группирует свой per-command список обратно во фрагменты (порядок сохраняется).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandGroupFragment {
    /// Имя группы команд (`NavigationPanelImportant`/`NavigationPanelOrdinary`/… — стандартная
    /// группа командного интерфейса).
    pub group: String,
    /// Полные имена команд фрагмента в ИСХОДНОМ порядке (`Catalog.X.StandardCommand.OpenList`).
    pub commands: Vec<String>,
}

/// Командный интерфейс подсистемы (`Subsystem`) — канонический IR текстового спутника
/// `CommandInterface.cmi` (EDT) / `Ext/CommandInterface.xml` (Designer). Несёт до четырёх
/// регионов в фиксированном порядке эмиссии: `commandsVisibility`, `commandsPlacement`,
/// `commandsOrder`, `subsystemsOrder` (Designer дополнительно эмитит производный
/// `GroupsOrder` — см. `pipeline::cmi_read`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandInterface {
    /// Видимость команд в ИСХОДНОМ порядке (позиционно значимо: cf-тело `<uuid>.1`
    /// эмитит команды в этом порядке).
    pub commands: Vec<CommandVisibility>,
    /// Регион `commandsPlacement` — размещение команд по группам (EDT-сгруппированная форма).
    /// В Designer каждая команда несёт `<Placement>Auto</Placement>` (единственное встреченное
    /// значение; ридер §1.0-проверяет его, райтер восстанавливает). Пусто ⇒ регион отсутствует.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placement: Vec<CommandGroupFragment>,
    /// Регион `commandsOrder` — порядок команд по группам (EDT-сгруппированная форма; Designer
    /// per-command БЕЗ `<Placement>`). Пусто ⇒ регион отсутствует.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<CommandGroupFragment>,
    /// Регион `subsystemsOrder` (EDT `<subsystems>` / Designer `<SubsystemsOrder><Subsystem>`)
    /// — порядок ДОЧЕРНИХ подсистем в интерфейсе, полными путями
    /// (`Subsystem.<Родитель>.Subsystem.<Дочерняя>`; witnessed ERP: 27 носителей, списки
    /// побайтно равны между диалектами). cf-тело `<uuid>.1` кодирует его регионом P3
    /// (`1, K, uuid×K` — witnessed erp.cf `Subsystem.CRMИМаркетинг`). Пусто ⇒ регион
    /// отсутствует (весь SSL-корпус).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subsystems_order: Vec<String>,
}

/// РАБОЧАЯ ОБЛАСТЬ НАЧАЛЬНОЙ СТРАНИЦЫ — канонический IR конфиг-сайдкара КОРНЯ
/// (`Ext/HomePageWorkArea.xml` Designer / `src/Configuration/HomePageWorkArea.hpwa` EDT;
/// читает/пишет `pipeline::config_interface_read`). cf-тело `<host>.8` —
/// `formats_cf::hpwa_body` (витнесс erp.cf `74cb37ec-….8`, 1 055 Б).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomePageWorkArea {
    /// Имя шаблона рабочей области (`<WorkingAreaTemplate>`/`<workingAreaTemplate>`).
    /// Witnessed-значение корпуса — `TwoColumnsVariableWidth`; cf-эмит §1.0-гейтит на него
    /// (коды lead/trailer других шаблонов не витнесснуты).
    pub template: String,
    /// Формы ЛЕВОЙ колонки (`<LeftColumn>` / первый `<columns>`) в исходном порядке.
    pub left: Vec<HomePageItem>,
    /// Формы ПРАВОЙ колонки (`<RightColumn>` / второй `<columns>`) в исходном порядке.
    pub right: Vec<HomePageItem>,
}

/// ОДНА форма рабочей области начальной страницы (`<Item>`/`<items>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomePageItem {
    /// Полное имя формы (`<Kind>.<Name>.Form.<FormName>` / `CommonForm.<Name>`) — cf
    /// резолвит в uuid формы через config-wide form roster.
    pub form: String,
    /// Высота элемента (`<Height>`/`<height>`; witnessed 13/13 — `10`).
    pub height: u64,
    /// Общая видимость (`<xr:Common>`/`<common>`; EDT `<visibility/>` == `false`).
    pub common_visible: bool,
    /// По-ролевые переопределения видимости (та же кодировка, что у
    /// [`CommandVisibility::role_values`]; в витнессе ERP отсутствуют — кодек общий с cmi).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role_values: Vec<RoleVisibility>,
}

/// ИНТЕРФЕЙС КЛИЕНТСКОГО ПРИЛОЖЕНИЯ (раскладка стандартных панелей) — канонический IR
/// конфиг-сайдкара КОРНЯ (`Ext/ClientApplicationInterface.xml` Designer, диалект
/// `managed-application/core` InterfaceLayouter / `src/Configuration/
/// ClientApplicationInterface.cai` EDT, атрибутный `g5.1c.ru/v8/dt/cai`). Канон = XML:
/// все id — внутренние id разметки (НЕ резолв по реестру). Диалекты СТРУКТУРНО РАЗНЫЕ
/// (как `.scc`): Designer ссылает панель на platform-fixed panelDef-uuid и несёт список
/// `<panelDef>`; EDT зовёт панели ИМЕНАМИ и перечисляет неразмещённые `<unset>`. Связка —
/// witnessed-таблица [`STANDARD_CLIENT_PANELS`] (имя ↔ platform-uuid); обе проекции
/// выводимы из канона (§1.6). cf-тело `<host>.b` — `formats_cf::cai_body`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientApplicationInterface {
    /// Верхняя группа панелей (`<top>`; cf-секция 1).
    pub top: Option<CaiGroup>,
    /// Левая группа панелей (`<left>`; cf-секция 3).
    pub left: Option<CaiGroup>,
}

/// Группа панелей интерфейса клиентского приложения (`<group id=…>` Designer /
/// `<top|left xsi:type="cai:CaiGroup" id=…>` EDT).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaiGroup {
    /// id группы (внутренний id разметки, дефисованный GUID — канон = XML).
    pub id: String,
    /// Размещённые панели группы в исходном порядке.
    pub panels: Vec<CaiPanel>,
}

/// ОДНА размещённая стандартная панель (`<panel id=…><uuid>…</uuid></panel>` Designer /
/// `<panels id=… name=…/>` EDT).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaiPanel {
    /// id ЭКЗЕМПЛЯРА панели (внутренний id разметки; побайтно равен между диалектами).
    pub id: String,
    /// ИМЯ стандартной панели (`ToolsPanel`/`OpenedPanel`/`SectionPanel`/… — EDT-имя;
    /// Designer-`<uuid>` выводится по [`STANDARD_CLIENT_PANELS`]).
    pub name: String,
}

/// Витнессированная таблица СТАНДАРТНЫХ ПАНЕЛЕЙ клиентского приложения
/// `(имя, platform-fixed panelDef-uuid)` В ПЛАТФОРМЕННОМ ПОРЯДКЕ (RE ERP: порядок ==
/// Designer-списку `<panelDef>` == порядку panelDef-хвоста cf-тела `<host>.b`; uuid'ы
/// связывают EDT-имена (`name=`/`<unset id=…>`) с Designer-`<uuid>`-ссылками). §1.0:
/// незнакомое имя/uuid — типизированный отказ (не guess).
pub const STANDARD_CLIENT_PANELS: &[(&str, &str)] = &[
    ("SectionPanel", "b553047f-c9aa-4157-978d-448ecad24248"),
    ("FavoritePanel", "13322b22-3960-4d68-93a6-fe2dd7f28ca3"),
    ("HistoryPanel", "c933ac92-92cd-459d-81cc-e0c8a83ced99"),
    ("OpenedPanel", "cbab57f2-a0f3-4f0a-89ea-4cb19570ab75"),
    ("FunctionsPanel", "b2735bd3-d822-4430-ba59-c9e869693b24"),
    ("ToolsPanel", "8e10648b-f52d-4ec2-b4dd-87de33778d95"),
];

/// Объект метаданных — РОВНО один тип IR на объект (§1.1 «одна сущность — один
/// тип»). Реквизиты, табличные части и подчинённые объекты — это `children`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataObject {
    /// Вид объекта (канонический код).
    pub kind: ObjectKind,
    /// Имя объекта (идентификатор 1С).
    pub name: String,
    /// Канонический UUID объекта (cross-format-связка, §3.5).
    pub uuid: Uuid,
    /// Платформенно-вычисляемая служебная информация (`producedTypes`/`InternalInfo`),
    /// если присутствует в источнике. `None` для видов без неё (CommonModule и т.п.).
    /// Часть КАНОНИЧЕСКОГО IR (edt==designer, §1.6); `skip_serializing_if` держит
    /// RON-снапшоты видов БЕЗ блока байт-идентичными (golden не ломается).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_info: Option<InternalInfo>,
    /// `thisNode` — GUID узла-идентичности плана обмена (EDT: root-атрибут `thisNode`;
    /// Designer: `<xr:ThisNode>` — первый ребёнок `<InternalInfo>`). Платформенная
    /// идентичность (как `uuid`), общая обоим форматам (X by construction). `None` для
    /// видов без неё (все, кроме ExchangePlan). `skip_serializing_if` держит RON-снапшоты
    /// прочих видов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub this_node: Option<Uuid>,
    /// Таблица прав роли (`Role`) — канонический IR текстового спутника
    /// `Rights.rights`/`Ext/Rights.xml` ([`crate::spec::metadata::role::RightsTable`]).
    /// Носится ОТДЕЛЬНО от `properties` (как `modules` — это спутник, не спек-свойство):
    /// whole-config read подгружает его из сайдкара (`pipeline::rights_read`), а `--to cf`
    /// эмитит тело прав `<uuid>.0` из него (`formats_cf::assemble_cf` RIGHTS_BODY_KINDS).
    /// `None` для видов без прав (все, кроме Role). `skip_serializing_if` держит RON-снапшоты
    /// прочих видов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights: Option<crate::spec::metadata::role::RightsTable>,
    /// Схема XDTO-пакета (`XDTOPackage`) — канонический (BOM-снятый) текст XML-спутника
    /// `Package.xdto` (EDT) / `Ext/Package.bin` (Designer). Носится ОТДЕЛЬНО от `properties`
    /// (как `rights` / `templates[].body` — это спутник, не спек-свойство): whole-config read
    /// подгружает его из сайдкара (`pipeline::xdto_read`), а `--to cf` эмитит тело схемы
    /// `<uuid>.0` = `BOM + xdto_schema` (`formats_cf::assemble_cf` XDTO_BODY_KINDS). RE-проверено
    /// на `coverage/cf/s5_reports_services.cf`: cf `<uuid>.0` inflate БАЙТ-В-БАЙТ == designer
    /// `Ext/Package.bin` == `BOM + edt Package.xdto`. `None` для видов без схемы (все, кроме
    /// XDTOPackage). `skip_serializing_if` держит RON-снапшоты прочих видов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xdto_schema: Option<Vec<u8>>,
    /// WSDL-описание WS-ссылки (`WSReference`) — канонический IR сайдкаров
    /// `Ext/WSDefinition.xml` + `Ext/<N>.xsd` (Designer) / `WsDefinitions.wsdl` + `<N>.xsd`
    /// (EDT), см. [`WsDefinition`]. Носится ОТДЕЛЬНО от `properties` (спутник, не
    /// спек-свойство): whole-config read подгружает его (`pipeline::ws_definition_read`), а
    /// `--to cf` эмитит тело `<uuid>.0` (вложенный контейнер `0.wsdl` + `<N>.xsd` —
    /// `formats_cf::ws_reference_body`). `None` для видов без WSDL (все, кроме WSReference) И
    /// для WSReference без сайдкара (наличие тела гейтится ФАЙЛОМ — RE erp.cf: `.0` есть
    /// ⟺ есть `Ext/WSDefinition.xml`). `skip_serializing_if` держит RON-снапшоты прочих
    /// видов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ws_definition: Option<WsDefinition>,
    /// КАРТА МАРШРУТА бизнес-процесса (`BusinessProcess`) — канонический (BOM-снятый) текст
    /// XML-спутника `Flowchart.scheme` (EDT) / `Ext/Flowchart.xml` (Designer). Носится ОТДЕЛЬНО
    /// от `properties` (как `rights`/`xdto_schema` — это спутник, не спек-свойство): whole-config
    /// read подгружает его из сайдкара (`pipeline::flowchart_read`), а `--to cf` эмитит тело
    /// схемы `<uuid>.7` (`formats_cf::flowchart_body`).
    ///
    /// КАНОН = EDT-текст (БЕЗ BOM, БЕЗ корневого `version=`, СТРУКТУРНЫЕ переводы строк CRLF,
    /// переводы строк ВНУТРИ текстовых узлов — LF, как того требует канонический IR). Оба
    /// диалекта дают РАВНЫЙ канон: Designer-файл = канон + BOM + `version="2.21"`, EDT-файл =
    /// канон с CRLF и в текстовых узлах (§1.6).
    ///
    /// `None` для видов без карты (все, кроме BusinessProcess) И для BusinessProcess без
    /// сайдкара (witnessed: платформа тогда НЕ эмитит `.7` вовсе).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flowchart: Option<Vec<u8>>,
    /// Командный интерфейс подсистемы (`Subsystem`) — канонический IR спутника
    /// `CommandInterface.cmi` (EDT) / `Ext/CommandInterface.xml` (Designer). Носится
    /// ОТДЕЛЬНО от `properties` (как `rights`/`xdto_schema` — это спутник, не спек-свойство):
    /// whole-config read подгружает его из сайдкара (`pipeline::cmi_read`), а `--to cf` эмитит
    /// тело командного интерфейса `<uuid>.1` из него (`formats_cf::assemble_cf` через
    /// `crate::cmi_body`). `None` для видов без него (все, кроме Subsystem) И для подсистем БЕЗ
    /// сайдкара (он ОПЦИОНАЛЕН — не всякая подсистема несёт `CommandInterface`).
    /// `skip_serializing_if` держит RON-снапшоты прочих видов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_interface: Option<CommandInterface>,
    /// Двоичное тело картинки вида `CommonPicture` ([`PictureBody`]) — спутник, подгружаемый
    /// whole-config read'ом (`pipeline::picture_read`) из `Picture.<ext>` (EDT) /
    /// `Ext/Picture/<file>`+`Ext/Picture.xml` (Designer). `None` для видов без картинки (все,
    /// кроме CommonPicture) и картинок-заглушек без файла. `skip_serializing_if` держит
    /// RON-снапшоты прочих видов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<PictureBody>,
    /// Картинки КОРНЯ конфигурации ([`ConfigPicture`]: `Splash`/`MainSectionPicture`) —
    /// спутники config-level `Ext`, подгружаемые whole-config read'ом (`pipeline::ext_read`)
    /// из `src/Configuration/<Слот>.<ext>` (EDT) / `Ext/<Слот>.xml`+`Ext/<Слот>/Picture.<ext>`
    /// (Designer). Пуст для всех видов, кроме корня `Configuration`, и для корней без картинок.
    /// `skip_serializing_if` держит RON-снапшоты прочих объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_pictures: Vec<ConfigPicture>,
    /// Бинарные конфиг-сайдкары КОРНЯ ([`ConfigBlob`]: `ParentConfigurations`/
    /// `MobileClientSignature`) — спутники config-level `Ext`, подгружаемые whole-config
    /// read'ом (`pipeline::ext_read`) из `src/Configuration/<файл>.bin` (EDT; ⚠️
    /// `MobileClientSign.bin` — EDT-имя короче) / `Ext/<Слот>.bin` (Designer). Байты —
    /// ВЕРБАТИМ (никакой перекодировки; byte-identical между диалектами, сверено ERP+SSL).
    /// Порядок — детерминированный по таблице слотов. Пуст для всех видов, кроме корня
    /// `Configuration`, и для корней без этих файлов (отсутствие честно).
    /// `skip_serializing_if` держит RON-снапшоты прочих объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config_blobs: Vec<ConfigBlob>,
    /// Состав автономной конфигурации ([`StandaloneContent`]) — СТРУКТУРНЫЙ конфиг-сайдкар
    /// КОРНЯ, подгружаемый whole-config read'ом (`pipeline::standalone_content_read`) из
    /// `Ext/StandaloneConfigurationContent.bin` (Designer) / `MobileApplicationContent.scc`
    /// (EDT, сиблинг `Configuration.mdo`). `--to cf` эмитит тело `<host>.f` с резолвом
    /// дот-имён в uuid'ы (`formats_cf::assemble`). `None` для всех видов, кроме корня, и
    /// для корней без файла (SSL/coverage — отсутствие честно, наличие гейтится файлом).
    /// `skip_serializing_if` держит RON-снапшоты прочих объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standalone_content: Option<StandaloneContent>,
    /// КОРНЕВОЙ командный интерфейс (порядок разделов) — конфиг-сайдкар КОРНЯ
    /// (`Ext/CommandInterface.xml` Designer / `src/Configuration/CommandInterface.cmi` EDT;
    /// ТОТ ЖЕ cmi-диалект, что у подсистемного сайдкара — витнесс ERP: только
    /// `subsystemsOrder` из 23 top-level подсистем). Читает/пишет
    /// `pipeline::config_interface_read`; `--to cf` эмитит тело `<host>.a` тем же
    /// cmi-кодеком (`formats_cf::cmi_body`). `None` для всех видов, кроме корня, и для
    /// корней без файла. `skip_serializing_if` держит RON-снапшоты байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_command_interface: Option<CommandInterface>,
    /// Командный интерфейс ОСНОВНОГО РАЗДЕЛА — конфиг-сайдкар КОРНЯ
    /// (`Ext/MainSectionCommandInterface.xml` Designer /
    /// `src/Configuration/MainSectionCommandInterface.cmi` EDT; тот же cmi-диалект, но
    /// Designer-`<Placement>` здесь `Manual`, не `Auto` — витнесс ERP 148/148). `--to cf`
    /// эмитит тело `<host>.9` cmi-кодеком с placementKind=1 (`formats_cf::cmi_body`).
    /// `None` для всех видов, кроме корня, и для корней без файла.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_section_command_interface: Option<CommandInterface>,
    /// РАБОЧАЯ ОБЛАСТЬ НАЧАЛЬНОЙ СТРАНИЦЫ ([`HomePageWorkArea`]) — конфиг-сайдкар КОРНЯ
    /// (`Ext/HomePageWorkArea.xml` Designer / `HomePageWorkArea.hpwa` EDT). `--to cf`
    /// эмитит тело `<host>.8` (`formats_cf::hpwa_body`, форм-ссылки — form roster).
    /// `None` для всех видов, кроме корня, и для корней без файла.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_page_work_area: Option<HomePageWorkArea>,
    /// ИНТЕРФЕЙС КЛИЕНТСКОГО ПРИЛОЖЕНИЯ ([`ClientApplicationInterface`]) — конфиг-сайдкар
    /// КОРНЯ (`Ext/ClientApplicationInterface.xml` Designer /
    /// `ClientApplicationInterface.cai` EDT). `--to cf` эмитит тело `<host>.b`
    /// (`formats_cf::cai_body`; id — из XML, реестр не нужен). `None` для всех видов,
    /// кроме корня, и для корней без файла.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_application_interface: Option<ClientApplicationInterface>,
    /// Страницы справки объекта ([`HelpPage`]) — спутник, подгружаемый whole-config read'ом
    /// (`pipeline::help_read`) из `Help/<lang>.html` (EDT) / `Ext/Help.xml` +
    /// `Ext/Help/<lang>.html` (Designer). Пуст для объектов без справки (наличие гейтится
    /// ФАЙЛОМ на диске — Designer-дескриптором `Ext/Help.xml` / EDT-каталогом `Help/`).
    /// `skip_serializing_if` держит RON-снапшоты прочих объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub help: Vec<HelpPage>,
    /// Вспомогательные файлы-ресурсы справки ([`HelpResource`]) из каталога `_files/` рядом
    /// со страницами (`Help/_files/…` EDT / `Ext/Help/_files/…` Designer) — спутник,
    /// подгружаемый тем же `pipeline::help_read`. §1.0: байт-вербатим, Designer `Ext/Help.xml`
    /// их НЕ объявляет. Пуст у объектов без ресурсов (почти все). `skip_serializing_if` держит
    /// RON-снапшоты прочих объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub help_resources: Vec<HelpResource>,
    /// РАСПИСАНИЕ регламентного задания ([`Schedule`]) — сайдкар `Schedule.schedule` (EDT) /
    /// `Ext/Schedule.xml` (Designer), подгружаемый `pipeline::schedule_read`; cf-сборщик эмитит
    /// его телом `<uuid>.0`. `None` у всех видов, кроме `ScheduledJob`, и у самих ScheduledJob'ов
    /// БЕЗ сайдкара (witnessed: 5 из 50 в SSL). `skip_serializing_if` держит RON-снапшоты прочих
    /// объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
    /// СОСТАВ СТИЛЯ ([`StyleRecord`]) — сайдкар `Style.style` (EDT) / `Ext/Style.xml`
    /// (Designer), подгружаемый `pipeline::style_read`; cf-сборщик эмитит его телом
    /// `<uuid>.0` (`formats_cf::style_body`). Пуст у всех видов, кроме `Style`, и у Style
    /// БЕЗ сайдкара (coverage-стаб — платформа хранит его дескриптором-только).
    /// `skip_serializing_if` держит RON-снапшоты прочих объектов байт-идентичными.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_records: Vec<StyleRecord>,
    /// Свойства по каноническому [`FieldId`], в порядке эмиссии спека (дефолты НЕ
    /// хранятся). `Vec`, а не map: порядок структурен и детерминирует
    /// сериализацию (§1.5). Уникальность ключей — инвариант движка, не типа.
    #[serde(serialize_with = "crate::ir::serialize_semantic_properties")]
    pub properties: Vec<(FieldId, PropertyValue)>,
    /// Подчинённые объекты (реквизиты, табличные части, формы-как-объекты и т.п.)
    /// в исходном порядке — сравнение позиционное (§3.2).
    pub children: Vec<MetadataObject>,
    /// Текстовые модули объекта.
    pub modules: Vec<Module>,
    /// Формы объекта (под-IR `FormItem`, §1.2 L1f) — ДЕРЕВО КОНТРОЛОВ отдельных форм.
    pub forms: Vec<FormItem>,
    /// ЦЕЛЫЕ тела форм объекта ([`NamedFormBody`] — весь под-IR `FormBody` + модуль формы),
    /// подгружаемые whole-config-конвейером из сайдкаров (`pipeline::form_read`: EDT
    /// `<dir>/Form.form`; Designer `<dir>/<Name>/Ext/Form.xml`). Носится ОТДЕЛЬНО от `forms`
    /// (то — лишь дерево контролов): whole-config write эмитит целевое тело формы через
    /// `write_form` в per-format-путь. `Vec` пуст у видов/объектов без формы-спутника (все
    /// пер-дескрипторные read'ы) — `skip_serializing_if` держит RON-снапшоты байт-идентичными.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_bodies: Vec<NamedFormBody>,
    /// Макеты/шаблоны объекта. Опаковые (MXL/картинки) живут как
    /// [`PropertyValue::Blob`] внутри свойств шаблона.
    pub templates: Vec<Template>,
    #[serde(default)]
    pub source_extensions: source_extensions::SourceExtensions,
}

impl MetadataObject {
    /// Создать пустой объект заданного вида/имени/uuid.
    pub fn new(kind: ObjectKind, name: impl Into<String>, uuid: Uuid) -> Self {
        MetadataObject {
            kind,
            name: name.into(),
            uuid,
            internal_info: None,
            this_node: None,
            rights: None,
            xdto_schema: None,
            ws_definition: None,
            flowchart: None,
            command_interface: None,
            picture: None,
            config_pictures: Vec::new(),
            config_blobs: Vec::new(),
            standalone_content: None,
            root_command_interface: None,
            main_section_command_interface: None,
            home_page_work_area: None,
            client_application_interface: None,
            help: Vec::new(),
            help_resources: Vec::new(),
            schedule: None,
            style_records: Vec::new(),
            properties: Vec::new(),
            children: Vec::new(),
            modules: Vec::new(),
            forms: Vec::new(),
            form_bodies: Vec::new(),
            templates: Vec::new(),
            source_extensions: source_extensions::SourceExtensions::default(),
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

/// Макет/шаблон объекта (имя + свойства; опаковые данные — через `Blob`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    /// Имя макета.
    pub name: String,
    /// Свойства макета по каноническому [`FieldId`] (тип макета, и т.п.).
    #[serde(serialize_with = "crate::ir::serialize_semantic_properties")]
    pub properties: Vec<(FieldId, PropertyValue)>,
    /// КАНОНИЧЕСКОЕ (BOM-снятое) тело макета, материализованное для эмиссии cf-тела
    /// `<uuid>.N` (аналог [`ModuleBody::Text`] — спутник объекта, не спек-свойство). Whole-
    /// config read подгружает его из сайдкара (`pipeline::template_read` — для TextDocument:
    /// EDT `Template.txt` без BOM / Designer `Ext/Template.txt` с BOM → BOM снимается → ОДИН
    /// канонический байт-поток, edt==designer), а `--to cf` эмитит тело `<uuid>.0` = `BOM +
    /// body` (`formats_cf::assemble_cf` TEMPLATE_BODY_KINDS). `None`, когда прочитан лишь
    /// дескриптор (или тип макета — структурный, не воспроизводимый byte-exact: MXL/DCS/HTML/
    /// AddIn/BinaryData несут ИНОЕ cf-кодирование, а не копию сайдкара). `skip_serializing_if`
    /// держит RON-снапшоты макетов без тела байт-идентичными.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Vec<u8>>,
    /// HTML-СТРАНИЦЫ HTMLDocument-макета: `(lang, канон-LF-тело без BOM)` В ПОРЯДКЕ МАНИФЕСТА.
    /// ERP-witnessed мультиязычность: 44×(en,ru) + 71×(ru) designer-манифестов; EDT
    /// `Template.htmldoc` несёт N `<pages lang="…"/>`; cf-упаковка
    /// `{5,N,(lang,{#base64:…})×N,0}` (RE erp.cf Партнеры/Макет). Для HTMLDocument тело
    /// живёт ЗДЕСЬ (`body` остаётся `None`); прочие типы макетов — в [`Template::body`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<(String, Vec<u8>)>,
    /// ФАЙЛЫ-сайдкары макета, `(ключ, сырые байты)` — вербатим-блобы, идентичные между
    /// диалектами; семантика ключа зависит от `templateType` (типы дизъюнктны — коллизий нет):
    /// * **GraphicalSchema** — КАРТИНКИ элементов схемы, ключ `"<ИмяЭлемента>/<ИмяФайла>"` в
    ///   порядке элементов (RE ERP, 37 макетов): EDT `Templates/<T>/Items/<Имя>/Picture.<ext>`,
    ///   Designer `Templates/<T>/Ext/Template/Items/<Имя>/Picture.<ext>`. Ключ =
    ///   `<ИмяЭлемента>/<имя из <Abs>>` — ровно то, чем cf-энкодер
    ///   (`formats_cf::flowchart_body`) адресует inline-картинку декорации.
    /// * **HTMLDocument** — приложенные файлы-ресурсы `_files/` рядом со страницами (RE ERP,
    ///   14 носителей/156 файлов), ключ = `/`-разделённый путь ОТНОСИТЕЛЬНО `_files/`,
    ///   порядок — сортированный обход (== порядку в cf-хвосте): EDT `<dir>/_files/<имя>`,
    ///   Designer `Ext/Template/_files/<имя>`; cf-упаковка — help-триплетный хвост
    ///   (`formats_cf::template_body`), читает/пишет `pipeline::blob_template_read`.
    ///
    /// Пусто у всех прочих типов макетов (`skip_serializing_if` держит RON-снапшоты
    /// байт-идентичными).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<(String, Vec<u8>)>,
}

/// Корень семантического IR — конфигурация как поток объектов (§1.1
/// «эффективность»: стриминг по одному объекту; здесь — материализованная форма
/// для тестов/обмена).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    /// ВЕРСИЯ ФОРМАТА ИСТОЧНИКА, из которого этот IR прочитан.
    ///
    /// Часть IR, а не только свойство файла: без неё по одному лишь мешку свойств НЕЛЬЗЯ
    /// ответить, было ли значение задано в источнике или достроено ридером, — а именно этот
    /// вопрос решают downgrade-гейт ([`crate::version::check_downgrade`]) и разрешение
    /// умолчаний ([`crate::resolve`]). Заполняется при чтении: Designer — корневой `version=`
    /// дескриптора, EDT — `DT-INF/PROJECT.PMF` (`Runtime-Version` → версия формата по
    /// реестру FORMATS.md §2), `.cf` — раскладка корневой записи
    /// (`formats_cf::root_record_version`).
    ///
    /// `None` — версию источника определить НЕ УДАЛОСЬ (нет файла-штампа, неизвестная
    /// платформа, синтетический IR из тестов). §1.0: это ЯВНОЕ «не знаю», а не подстановка
    /// «похожей» версии; потребители обязаны трактовать `None` как «судить по одному IR».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version: Option<crate::version::FormatVersion>,
    /// Свойства корневого объекта конфигурации по каноническому [`FieldId`].
    #[serde(serialize_with = "crate::ir::serialize_semantic_properties")]
    pub properties: Vec<(FieldId, PropertyValue)>,
    /// Объекты конфигурации в исходном порядке.
    pub objects: Vec<MetadataObject>,
}

impl Configuration {
    /// Создать пустую конфигурацию (версия источника — «не известна», см.
    /// [`Configuration::source_version`]).
    pub fn new() -> Self {
        Configuration {
            source_version: None,
            properties: Vec::new(),
            objects: Vec::new(),
        }
    }

    /// Та же конфигурация с проставленной версией формата источника.
    pub fn with_source_version(mut self, version: Option<crate::version::FormatVersion>) -> Self {
        self.source_version = version;
        self
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}
