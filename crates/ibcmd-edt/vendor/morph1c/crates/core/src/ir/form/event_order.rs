//! ПЛАТФОРМЕННЫЙ ПОРЯДОК форм-/табличных событий: guid-таблицы типов событий + слияние
//! двух локусов IR (`events` ⊕ `<extInfo><handlers>`) в ОДИН Designer-`<Events>`.
//!
//! # Зачем это в `core`, а не в `formats-cf`
//! Guid типа события — ПЛАТФОРМЕННАЯ идентичность события, и она управляет ДВУМЯ вещами
//! сразу:
//! * cf: реестр `{N,(guid,"H")×N,1,0,(guid,0,1)×N}` — записи ОТСОРТИРОВАНЫ по guid;
//! * Designer-XML: единый `<Events>` — записи ОТСОРТИРОВАНЫ ПО ТОМУ ЖЕ guid.
//!
//! Второе намайнено в r34 по ПОЛНОМУ корпусу — 11 485 форм с событиями
//! (SSL `designer_8.5.1` 813 + ERP `designer_8.3.27` 10 672), правило держится
//! **11 485/11 485, контрпримеров 0**; множество имён в Designer-`<Events>` РАВНО объединению
//! EDT-локусов (`SET_MISMATCH` 0), а каждый локус EDT сам уже guid-отсортирован
//! (0 нарушений в обоих корпусах) ⇒ слияние = устойчивое СЛИЯНИЕ двух сортированных списков.
//!
//! Поэтому таблица перестала быть cf-эксклюзивным знанием: её читают ОБА формат-слоя
//! (`formats-cf` — реестр, `formats-xml` — Designer-`<Events>`), а `formats-cf` зависит от
//! `formats-xml` ⇒ единственное общее место — `core`.
//!
//! # Ассиметрия локусов (статья B `docs/X_LEDGER_FORMS.md`)
//! EDT держит СОБСТВЕННЫЕ обработчики корневого extInfo в `<extInfo><handlers>`
//! ([`FormRootExtInfo::events`]), а Designer сливает их в корневой `<Events>`; то же у
//! ТАБЛИЦЫ-динамического-списка ([`DynamicListExt::events`] ⟺ `<Events>` таблицы).
//! [`merge_form_events`] / [`merge_table_events`] воспроизводят это слияние.
//!
//! [`FormRootExtInfo::events`]: crate::ir::FormRootExtInfo::events
//! [`DynamicListExt::events`]: crate::ir::DynamicListExt::events

use crate::ir::FormEvent;

/// Платформенные guid ФОРМ-уровневых типов событий — FD-намайнено по всем 876 телам форм SSL
/// (`{N,(guid,"Handler")×N,1,0,(guid,0,1)×N}`, записи отсортированы по guid; каждый набор
/// обработчиков совпал с объединением IR `events` + `root_ext_info.events` 876/876).
/// `BeforeWrite` / `BeforeWriteAtServer` под `form:DocumentFormExtInfo` несут ДРУГОЙ guid —
/// см. [`DOCUMENT_EVENT_GUIDS`] и [`form_event_guid`].
pub const FORM_EVENT_GUIDS: &[(&str, &str)] = &[
    ("AfterWrite", "047d4d09-961c-4bdc-8519-eef10674c35b"),
    (
        "OnSaveDataInSettingsAtServer",
        "1952a54f-35ad-4928-902f-df212ab38ca3",
    ),
    ("ChoiceProcessing", "1d632984-de3c-4b4b-ad9f-d69682a10182"),
    (
        "BeforeLoadVariantAtServer",
        "1dd89674-8b50-4240-9899-e3426b79cb02",
    ),
    ("AfterWriteAtServer", "213d1900-dcad-4616-9f20-3f077156a40f"),
    (
        "NotificationProcessing",
        "3699f6a3-9a2a-4c82-a775-6ff4824a08ca",
    ),
    ("OnReadAtServer", "390d5e4b-e732-4c88-8748-9e211a416984"),
    ("OnOpen", "3ccc650e-f631-4cae-8e33-3eaac610b5f9"),
    (
        "BeforeLoadUserSettingsAtServer",
        "40925042-2517-455b-a600-d68282829334",
    ),
    (
        "OnSaveVariantAtServer",
        "499bb7af-6262-4de4-819f-ef264d1a20ec",
    ),
    ("BeforeClose", "52dbb775-1631-4fd5-8c55-1615b5881dac"),
    ("ExternalEvent", "5426e344-5740-4f23-99c1-99179a200dc5"),
    ("URLGetProcessing", "674956b3-e469-4fdc-acf5-24ebf88cf7ab"),
    ("OnReopen", "6b3175a5-c143-4179-a670-ef231dc0a688"),
    (
        "OnLoadDataFromSettingsAtServer",
        "79cea13e-f6fb-4483-905d-713326405771",
    ),
    (
        "OnLoadUserSettingsAtServer",
        "7b15b3db-1cd0-4e1d-a74b-2c972c9e2226",
    ),
    (
        "OnLoadVariantAtServer",
        "87ce636e-9de6-4e42-9395-f0f189d08397",
    ),
    (
        "NavigationProcessing",
        "93dfba16-26db-46f8-acb5-4f92f50c855f",
    ),
    (
        "OnSaveUserSettingsAtServer",
        "961ee7c6-0327-422b-adcb-97a90c46753d",
    ),
    ("BeforeWrite", "9cc34712-da5f-4faa-a653-343d2085fbe8"),
    ("OnCreateAtServer", "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b"),
    (
        "BeforeWriteAtServer",
        "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6",
    ),
    ("OnWriteAtServer", "c1bc0d3e-d35e-4207-a06b-ece68ed25314"),
    ("OnClose", "ca21cd18-35b2-4281-b5c8-016ecc8da8ac"),
    (
        "OnMainServerAvailabilityChange",
        "d6b86f20-722b-4fe6-83fa-85c6aa4c1fe5",
    ),
    (
        "OnUpdateUserSettingSetAtServer",
        "d817bccf-504e-4133-a79a-dd16e3a4df73",
    ),
    ("URLProcessing", "e0cd9bdf-88fa-428c-9f1f-86f7f73b11e2"),
    (
        "FillCheckProcessingAtServer",
        "e73d6384-49d2-4885-a752-a674d6ff7742",
    ),
    // Намайнено по реестрам оракула (`probe_mine`, `MINE form-events …`; 3 носителя, 0
    // конфликтов). NB `Catalog/ПрофилиГруппДоступа/ФормаЭлемента` привязывает И это событие,
    // И `OnLoadDataFromSettingsAtServer` к ОДНОЙ процедуре — два guid развёл третий носитель,
    // чей обработчик назван `Перед…`.
    (
        "BeforeLoadDataFromSettingsAtServer",
        "e773807c-0c0c-4689-a093-231ddcd6409f",
    ),
    // --- ERP-намайненный блок (оракул = .fixtures/ERP/cf/erp.cf; ДОПИСАН, строки выше не
    // тронуты). Считано с головной EVENTS-области каждого тела `<uuid>.0` и спарено через имя
    // обработчика, объявленное designer-XML. Тот же проход независимо перевывел 14 строк выше
    // из ERP, побайтно тождественно (0 конфликтов).
    // 241c61f6-19af-4e97-9310-b1c0d210e50c Catalog/ВидыЗапасов/ФормаВводаВидовЗапасов ·
    //   обработчик `ОбработкаЗаписиНового`. NB это ФОРМ-уровневый guid; табличный
    //   `NewWriteProcessing` — ДРУГОЙ guid (`ce67decf…`, см. [`TABLE_EVENT_GUIDS`]).
    (
        "NewWriteProcessing",
        "3b644f4f-055f-4808-bdc6-a50ce895e4d9",
    ), // n=8
    // ЕДИНСТВЕННЫЙ НОСИТЕЛЬ в ERP (подтверждение ≥2 из этого корпуса невозможно):
    // 35d00e81-aef4-4bd1-a4e7-f8272c947fc9 Report/МониторЦелевыхПоказателей/
    //   ФормаМониторЦелевыхПоказателей · обработчик `ПриИзмененииПараметровЭкрана`
    (
        "OnChangeDisplaySettings",
        "b98da5a8-349c-4159-a6a8-17a34ceb10ec",
    ), // n=1
    // ЕДИНСТВЕННЫЙ НОСИТЕЛЬ в ERP: fc5615ea-3ec1-4799-9364-2f684d069df4 Catalog/Новости/
    //   ФормаПросмотраНовостей · обработчик `ОбработкаПолученияСпискаНавигационныхСсылок`
    (
        "URLListGetProcessing",
        "44498116-1641-4bfa-ae33-86e53c205797",
    ), // n=1
    // Exact installed EDT runtime .type identities, independently bound to the
    // 8.3.27/8.5.1 platform jars by sdk_event_owners::installed_sdk_tables_agree.
    // Root ActivationProcessing takes precedence over the Task extension's
    // same-named event, as in EventHandlerXmlPartReader's owner exclusion.
    ("ActivationProcessing", "21da18af-c6f6-4433-9fbc-86b3b22dd7b3"),
    ("AddInDetachmentOnError", "ecc0d8bf-c3da-4af5-a7ec-836635143bf9"),
    ("BeforeReopenFromOtherServer", "07884f97-281c-45ed-bb58-b1df2529c968"),
    ("CollaborationSystemUsersAutoComplete", "54bc68a5-4ffc-430c-8dd7-2d07cbb48ae5"),
    ("CollaborationSystemUsersChoiceFormGetProcessing", "a00bb88f-4de1-4a8f-a720-1bc9b74689db"),
    ("OnPasteFromClipboard", "2c5f182c-1a2b-4fe1-a340-71979d5c39a8"),
    ("OnReopenFromOtherServer", "14e2ecfe-38f1-402c-8107-44c13851e739"),
    ("OnClientApplicationSuspend", "1a97ed33-7804-41cc-abfe-9d45f937cb3c"),
    ("OnClientApplicationResume", "8edde14c-57d9-480d-8119-8eb26ed7dde2"),
    ("ValueChoice", "0bf5cb1e-85d7-4344-8e8e-e8e131006339"),
    ("BeforeStart", "36205ca4-af87-4708-b594-00ffb647b887"),
    ("BeforeExecute", "ea0a9886-1607-44fe-a446-2cc57548f57d"),
    ("AfterComposeResult", "b634e40b-c7cd-471b-9d2d-03406a7ee2b2"),
    ("OnComposeResult", "acb39a89-2fa6-4a11-a764-5597b67f5fff"),
    ("OnSettingsChange", "58a9c022-69bc-495e-aab1-32be2210fb79"),
];

/// Переопределения для двух имён событий, чей guid зависит от вида корневого extInfo
/// (намайнено: носители `form:DocumentFormExtInfo` используют ЭТИ, любой другой вид — строку
/// из [`FORM_EVENT_GUIDS`]).
///
/// Переопределение load-bearing и для ПОРЯДКА Designer-`<Events>`: в 603 документных формах
/// (SSL 10 + ERP 593) не-документный guid дал бы ДРУГОЙ порядок, и он совпал с эталоном
/// ровно при документном (980/980 документных форм с событиями).
pub const DOCUMENT_EVENT_GUIDS: &[(&str, &str)] = &[
    ("BeforeWrite", "8a5894c9-d2ff-4c1d-b433-89cc352bbfbc"),
    (
        "BeforeWriteAtServer",
        "8f42e083-be92-4102-b1f0-fa58452c1a63",
    ),
];

/// 31 витнессированный guid ТАБЛИЧНЫХ типов событий — 24 FD-намайнено по корпусу SSL,
/// 7 дописано с ERP-оракула (см. блок-коммент внутри). `codecs::events_registry` (cf)
/// резолвит обработчики через эту таблицу; НЕперечисленное имя — типизированный отказ, а не
/// угаданный guid.
///
/// Табличное guid-пространство НЕ равно форм-уровневому: `BeforeLoadUserSettingsAtServer` —
/// `c41e7b98…` у Таблицы и `40925042…` у Формы ([`FORM_EVENT_GUIDS`]); заимствовать
/// перекрёстно нельзя. (`URLGetProcessing` СОВПАДАЕТ с форм-уровневым guid: витнессировано,
/// не предположено.)
///
/// Порядок Designer-`<Events>` ТАБЛИЦЫ — тот же guid-порядок: намайнено по 604 таблицам-ДС
/// (SSL 14 + ERP 590), **604/604, контрпримеров 0**; конкатенация «обычные, потом
/// собственный `OnGetDataAtServer`» ошибочна в 105 из них (SSL 8 + ERP 97).
pub const TABLE_EVENT_GUIDS: &[(&str, &str)] = &[
    ("AfterDeleteRow", "de65638d-a806-4a76-bc10-f62bbc86e0e7"),
    ("BeforeAddRow", "2391e7b8-7235-45d7-ab7e-6ff3dc086396"),
    ("BeforeCollapse", "a7a9dc42-29b6-4c5b-8980-6d0b87149bdd"),
    ("BeforeDeleteRow", "2ccfdec5-583d-4eca-8319-e55de492665a"),
    ("BeforeEditEnd", "4d88756d-bad4-4fde-92e1-c1f1402ac6b2"),
    ("BeforeExpand", "7c39b7bc-db0f-4410-9d98-8e5b7896995e"),
    ("BeforeRowChange", "ab930362-ff94-4dcb-ad16-188805d23e3c"),
    ("ChoiceProcessing", "8bfdb5eb-62dc-4851-8a2c-e983526356bf"),
    ("Drag", "8ad48496-8d0b-4f6c-ae48-99d95227884b"),
    ("DragCheck", "0d644ff6-443b-4390-86fa-7f9105e42711"),
    ("DragEnd", "cb286ab3-3a1c-40d2-a232-6e64f624ccec"),
    ("DragStart", "6d4d6747-a823-4f61-ab31-a426572f2c6c"),
    ("NewWriteProcessing", "ce67decf-16b8-4d61-b347-4e6a063580dc"),
    ("OnActivateCell", "f228b12f-d892-4925-b338-695617357b32"),
    ("OnActivateField", "6e973761-8683-47fa-a609-4e230950294d"),
    ("OnActivateRow", "60edb81d-887b-478e-94ee-7fef2b13393d"),
    ("OnChange", "fe115cc8-9e33-4684-a166-bd5136fe7a9f"),
    // СОБСТВЕННЫЙ обработчик таблицы-ДИНАМИЧЕСКОГО-СПИСКА (EDT `<extInfo><handlers>`) — то же
    // пространство, тот же guid; ПЕРФЕКТНАЯ биекция 14/14 по DL-корпусу SSL, 0 контрпримеров.
    ("OnGetDataAtServer", "97365900-eadf-4dfd-a9aa-fbb9ecabd079"),
    ("OnEditEnd", "01d80ddd-dce5-4db3-beb5-f63c97cb05b9"),
    ("OnHover", "c676f87f-6c33-4dba-aad8-0526726d1bcf"),
    (
        "OnSelectedRowsSetChange",
        "147fd867-8f22-4463-939d-4b48c5860c89",
    ),
    ("OnStartEdit", "b3c10170-c5ff-4cba-b537-679e1c872b45"),
    ("Selection", "1282f000-23b6-4887-87f4-9e8e79db3d32"),
    ("ValueChoice", "0d8cf5b0-55eb-4d1e-960a-22c160210945"),
    // --- ERP-намайненный блок (оракул = .fixtures/ERP/cf/erp.cf; ДОПИСАН, 24 строки выше не
    // тронуты). Каждая строка считана с EVENTS-области тела `<uuid>.0` и спарена с событием
    // через ИМЯ обработчика, объявленное designer-XML; строка принималась только когда РОВНО
    // ОДНА область в теле несла набор обработчиков хозяина. Тот же проход независимо перевывел
    // 15 из 24 строк выше и воспроизвёл их побайтно (0 конфликтов).
    //
    // n = независимых Таблиц, витнессированных по корпусу ERP.
    // d3291b71-3619-4d53-a8b4-182a031122ac AccumulationRegister/ДетализацияПартийТоваровДляНДСиУСН/
    //   ФормаСписка · таблица `Список` · `СписокПередЗагрузкойПользовательскихНастроекНаСервере`
    (
        "BeforeLoadUserSettingsAtServer",
        "c41e7b98-098c-433e-8ac3-56ec2a2c49e2",
    ), // n=21
    // 6f8812a8-b4a2-41dd-a290-fe228421b0f7 AccumulationRegister/ФактическиеОтпуска/ФормаСписка ·
    //   `Список` · `СписокПриОбновленииСоставаПользовательскихНастроекНаСервере`
    (
        "OnUpdateUserSettingSetAtServer",
        "e91128e6-621d-4dc8-b12e-bd65aeb37e2d",
    ), // n=14
    // af65b7af-84b4-47a9-be07-44fcd51f7a76 Catalog/ПодразделенияОрганизаций/ФормаСписка ·
    //   `Список` · `СписокПриЗагрузкеПользовательскихНастроекНаСервере`
    (
        "OnLoadUserSettingsAtServer",
        "336b3ee5-d67f-4651-b098-e2c53f8317e2",
    ), // n=7
    // 126f2266-eed1-4e6d-b4cd-eff01a000d23 DocumentJournal/УведомленияОВыплатеПособий/ФормаСписка
    //   и 44c5c7c5-4d59-41dd-bd30-12ba86269549 Catalog/Сотрудники/ФормаВыбора · оба `Список` ·
    //   `СписокПриСохраненииПользовательскихНастроекНаСервере`
    (
        "OnSaveUserSettingsAtServer",
        "a73dae96-734d-42e4-8ae7-b70249ecd233",
    ), // n=2 (ЕДИНСТВЕННЫЕ два носителя в ERP)
    // 607fc0a8-9c44-4163-a17a-ecfd9abf5f6c DataProcessor/ИнтерфейсДокументовЭДО/
    //   АрхивЭлектронныхДокументов · `Список` · `СписокОбработкаЗапросаОбновления`
    (
        "RefreshRequestProcessing",
        "ff33c4d6-a0db-4906-992e-37b3f44cd97a",
    ), // n=9
    // ЕДИНСТВЕННЫЙ НОСИТЕЛЬ в ERP (подтверждение ≥2 из этого корпуса НЕВОЗМОЖНО, не пропущено):
    // be82300a-cb6c-4f20-a2c9-9284799815b3 DataProcessor/ИнтерфейсДокументовЭДО/
    //   ТекущиеДелаПоЭДОЛегкийКлиентЭДО · `Черновики` ·
    //   `ЧерновикиОбработкаПолученияНавигационнойСсылки`. Его область стоит между соседними
    //   табличными областями, каждый обработчик в ней с префиксом `Черновики…`, а форм-уровневая
    //   область не несёт URL-события ⇒ атрибуция хозяина однозначна.
    ("URLGetProcessing", "674956b3-e469-4fdc-acf5-24ebf88cf7ab"), // n=1
    // ЕДИНСТВЕННЫЙ НОСИТЕЛЬ в ERP: 1e5a6832-7002-4cd1-97e2-3815cbde3b25 DataProcessor/
    //   РаботаСНоменклатурой/ЗагрузкаНоменклатуры · `Список` · `СписокПриСменеТекущегоРодителя`
    (
        "OnCurrentParentChange",
        "2971b9a9-1724-4f34-aaa4-f3db584c3ca0",
    ), // n=1
];

/// Guid ФОРМ-уровневого типа события; `document_ext` — корневой extInfo формы есть
/// `form:DocumentFormExtInfo` (тогда сначала смотрится [`DOCUMENT_EVENT_GUIDS`]).
/// `None` — имя НЕ витнессировано (guid не угадывается, §1.0).
pub fn form_event_guid(name: &str, document_ext: bool) -> Option<&'static str> {
    if document_ext {
        if let Some((_, g)) = DOCUMENT_EVENT_GUIDS.iter().find(|(n, _)| *n == name) {
            return Some(g);
        }
    }
    FORM_EVENT_GUIDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, g)| *g)
}

/// Guid ТАБЛИЧНОГО типа события. `None` — имя НЕ витнессировано.
pub fn table_event_guid(name: &str) -> Option<&'static str> {
    TABLE_EVENT_GUIDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, g)| *g)
}

/// Слить два локуса обработчиков в ОДИН Designer-`<Events>`: устойчивая сортировка по guid
/// типа события (правило намайнено по 11 485 формам, контрпримеров 0).
///
/// `None` ⇒ хотя бы одно имя НЕ витнессировано: порядок такого события НЕИЗВЕСТЕН, и он НЕ
/// угадывается (§1.0). Вызывающий обязан деградировать до сохранения исходного порядка —
/// cf-энкодер на таком имени всё равно даёт типизированный отказ, так что «неизвестное имя»
/// не может дожить до отгруженной конфигурации.
fn merge_by_guid<'a>(
    own: &'a [FormEvent],
    ext: &'a [FormEvent],
    guid: impl Fn(&str) -> Option<&'static str>,
) -> Option<Vec<&'a FormEvent>> {
    // ext-локус ПУСТ ⇒ сливать нечего: отдаём `own` КАК ЕСТЬ. Это не оптимизация, а §1.3
    // («сохранение исходного порядка»): порядок ОДНОГО локуса — вход, а не наша функция.
    // Designer-ридер всегда даёт пустой ext ⇒ Designer→Designer байт-в-байт по построению,
    // а не «по счастливой сортируемости»; и рукотворный IR (тесты, form-compile) не
    // переупорядочивается.
    if ext.is_empty() {
        return Some(own.iter().collect());
    }
    let mut keyed: Vec<(&'static str, &'a FormEvent)> = Vec::with_capacity(own.len() + ext.len());
    for ev in own.iter().chain(ext.iter()) {
        keyed.push((guid(&ev.name)?, ev));
    }
    // Устойчиво: равных guid в одном `<Events>` не бывает (cf-реестр такое отвергает), но
    // устойчивость делает слияние ДЕТЕРМИНИРОВАННЫМ.
    keyed.sort_by_key(|(g, _)| *g);
    Some(keyed.into_iter().map(|(_, e)| e).collect())
}

/// Единый корневой `<Events>` формы: `body.events` ⊕ `root_ext_info.events` в платформенном
/// (guid) порядке. Деградирует до конкатенации, если встретилось невитнессированное имя.
pub fn merge_form_events<'a>(
    own: &'a [FormEvent],
    ext: &'a [FormEvent],
    document_ext: bool,
) -> Vec<&'a FormEvent> {
    merge_by_guid(own, ext, |n| form_event_guid(n, document_ext))
        .unwrap_or_else(|| own.iter().chain(ext.iter()).collect())
}

/// Единый `<Events>` ТАБЛИЦЫ: `item.events` ⊕ `dynamic_list_ext.events` в платформенном
/// (guid) порядке. Деградирует до конкатенации на невитнессированном имени.
pub fn merge_table_events<'a>(own: &'a [FormEvent], ext: &'a [FormEvent]) -> Vec<&'a FormEvent> {
    merge_by_guid(own, ext, table_event_guid)
        .unwrap_or_else(|| own.iter().chain(ext.iter()).collect())
}

#[cfg(any())]
mod tests {
    use super::*;

    fn ev(name: &str) -> FormEvent {
        FormEvent {
            name: name.to_string(),
            handler: format!("H{name}"),
        }
    }

    fn names(v: Vec<&FormEvent>) -> Vec<&str> {
        v.into_iter().map(|e| e.name.as_str()).collect()
    }

    /// Витнесс задачи: `InformationRegister/ПубличныеИдентификаторыСинхронизируемыхОбъектов/
    /// ФормаЗаписи` — `OnOpen` (обычное) + `BeforeWrite` (собственное extInfo) ⇒ `OnOpen`
    /// первым (3ccc… < 9cc3…).
    #[test]
    fn form_merge_witness_record_form() {
        let own = vec![ev("OnOpen")];
        let ext = vec![ev("BeforeWrite")];
        assert_eq!(
            names(merge_form_events(&own, &ext, false)),
            ["OnOpen", "BeforeWrite"]
        );
    }

    /// Витнесс перемежения: `Catalog/…/ФормаЭлемента` — extInfo-события ложатся ВНУТРЬ
    /// обычных, не в хвост (правило = guid-порядок, а не «сначала обычные»).
    #[test]
    fn form_merge_interleaves() {
        let own = vec![
            ev("NotificationProcessing"),
            ev("OnOpen"),
            ev("OnCreateAtServer"),
        ];
        let ext = vec![
            ev("AfterWrite"),
            ev("AfterWriteAtServer"),
            ev("OnReadAtServer"),
            ev("BeforeWrite"),
            ev("BeforeWriteAtServer"),
        ];
        assert_eq!(
            names(merge_form_events(&own, &ext, false)),
            [
                "AfterWrite",
                "AfterWriteAtServer",
                "NotificationProcessing",
                "OnReadAtServer",
                "OnOpen",
                "BeforeWrite",
                "OnCreateAtServer",
                "BeforeWriteAtServer",
            ]
        );
    }

    /// Документное переопределение guid МЕНЯЕТ порядок: `BeforeWriteAtServer` уходит ПЕРЕД
    /// `OnCreateAtServer` (8f42… < 9f2e…), тогда как не-документный bf0a… — после.
    #[test]
    fn document_override_changes_order() {
        let own = vec![ev("OnCreateAtServer")];
        let ext = vec![ev("BeforeWriteAtServer")];
        assert_eq!(
            names(merge_form_events(&own, &ext, false)),
            ["OnCreateAtServer", "BeforeWriteAtServer"]
        );
        assert_eq!(
            names(merge_form_events(&own, &ext, true)),
            ["BeforeWriteAtServer", "OnCreateAtServer"]
        );
    }

    /// Таблица-ДС: `OnGetDataAtServer` (9736…) встаёт МЕЖДУ `OnActivateRow` (60ed…) и
    /// `OnHover` (c676…) — витнесс `Catalog/…/ФормаВыбора`.
    #[test]
    fn table_merge_dynamic_list_handler_is_not_appended() {
        let own = vec![ev("OnActivateRow"), ev("OnHover"), ev("OnChange")];
        let ext = vec![ev("OnGetDataAtServer")];
        assert_eq!(
            names(merge_table_events(&own, &ext)),
            ["OnActivateRow", "OnGetDataAtServer", "OnHover", "OnChange"]
        );
    }

    /// Невитнессированное имя ⇒ порядок НЕ угадывается: сохраняем конкатенацию.
    #[test]
    fn unwitnessed_name_falls_back_to_concat() {
        let own = vec![ev("OnCreateAtServer"), ev("ЧтоТоНевиданное")];
        let ext = vec![ev("AfterWrite")];
        assert_eq!(
            names(merge_form_events(&own, &ext, false)),
            ["OnCreateAtServer", "ЧтоТоНевиданное", "AfterWrite"]
        );
    }

    /// ПУСТОЙ ext-локус (весь Designer→Designer путь) НЕ переупорядочивает `own` — иначе
    /// событие контрола, чьё имя случайно есть в ТАБЛИЧНОМ пространстве (`OnChange`,
    /// `ChoiceProcessing`), уехало бы, сломав byte-exact.
    #[test]
    fn empty_ext_locus_preserves_own_order() {
        let own = vec![ev("OnChange"), ev("ChoiceProcessing")];
        assert_eq!(
            names(merge_table_events(&own, &[])),
            ["OnChange", "ChoiceProcessing"]
        );
        assert_eq!(
            names(merge_form_events(&own, &[], false)),
            ["OnChange", "ChoiceProcessing"]
        );
    }

    /// Оба guid-пространства РАЗЛИЧНЫ и не должны заимствоваться перекрёстно.
    #[test]
    fn form_and_table_guid_spaces_differ() {
        assert_eq!(
            form_event_guid("BeforeLoadUserSettingsAtServer", false),
            Some("40925042-2517-455b-a600-d68282829334")
        );
        assert_eq!(
            table_event_guid("BeforeLoadUserSettingsAtServer"),
            Some("c41e7b98-098c-433e-8ac3-56ec2a2c49e2")
        );
    }
}
