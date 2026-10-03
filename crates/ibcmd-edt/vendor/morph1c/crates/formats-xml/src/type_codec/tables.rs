//! Таблицы canon<->QName и предикаты классификации type-id (диалект-нейтральные).

// --- canon↔QName таблица (§1.2/§3.1) ----------------------------------------

/// Канон id ↔ Designer QName для XSD-примитивов (имя ДРУГОЕ, не только префикс).
/// `(canon, qname)`.
pub(crate) const PRIMITIVES: &[(&str, &str)] = &[
    ("String", "xs:string"),
    ("Number", "xs:decimal"),
    ("Date", "xs:dateTime"),
    ("Boolean", "xs:boolean"),
];

/// Канон id платформенных типов с НЕТРИВИАЛЬНЫМ local-name алиасом (§1.2 ТРАП).
/// `(canon, qname)`. Только те, у кого local-name отличается; остальные
/// платформенные — механический `v8:` + тот же local-name.
const PLATFORM_ALIASES: &[(&str, &str)] = &[
    ("ComparisonType", "ent:ComparisonType"),
    // EDT `ValueList` ↔ Designer `v8:ValueListType` (local-name РАЗНЫЙ!).
    ("ValueList", "v8:ValueListType"),
    // EDT `AnyRef` ↔ Designer `cfg:AnyIBRef` (local-name РАЗНЫЙ — Designer-имя для
    // «любая ссылка ИБ»; сверено 18× в IR-корпусе). Хостится type-set'ом (`is_type_set`
    // ловит `AnyRef`); alias-таблица берёт верх над generic `cfg:`-веткой в обе стороны.
    ("AnyRef", "cfg:AnyIBRef"),
    // EDT `DataCompositionSettingsComposer` ↔ Designer `dcsset:SettingsComposer` (префикс
    // `dcsset:` И local-name РАЗНЫЕ). Designer-хост `<v8:Type>` несёт ИНЛАЙН-объявление
    // `xmlns:dcsset` (см. `INLINE_NS_QNAMES`/`inline_ns_for_qname`) — в DataProcessor-корпусе
    // всегда инлайн (2/2). Сверено: `v8:DataCompositionSettingsComposer` в Designer НЕ
    // встречается (0×), только `dcsset:SettingsComposer` (32× по SSL).
    ("DataCompositionSettingsComposer", "dcsset:SettingsComposer"),
    // EDT `ConstantsSet` ↔ Designer `cfg:ConstantsSet` (префикс `cfg:`; `cfg` объявлен в
    // ENVELOPE корня — БЕЗ инлайн-ns). Сверено: `v8:ConstantsSet` в Designer НЕ встречается.
    ("ConstantsSet", "cfg:ConstantsSet"),
    // EDT `ReportBuilder` ↔ Designer `cfg:ReportBuilder` (префикс `cfg:` из envelope корня —
    // БЕЗ инлайн-ns). Сверено: `v8:ReportBuilder` НЕ встречается, только `cfg:ReportBuilder`.
    ("ReportBuilder", "cfg:ReportBuilder"),
    // `Chart` перенесён в [`AUTO_NS_TYPES`]: его Designer-префикс АВТО-ГЕНЕРИРУЕМЫЙ
    // `d{N}p1` (N = глубина host-элемента): дескриптор ОценкаПроизводительности несёт
    // `d7p1:Chart` (глубина 7), а ФОРМА ПодборЦелевогоВремениКлючевойОперации —
    // `d5p1:Chart` (Form>Attributes>Attribute>Type>v8:Type = глубина 5).
    // EDT `FormattedString` ↔ Designer `v8ui:FormattedString` (префикс `v8ui:` —
    // объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns). Тип форматированной строки,
    // встречается в параметрах форм (`CommonForm.ФормаПроизвольногоСообщения`). Сверено:
    // EDT bare `FormattedString` ↔ Designer `v8ui:FormattedString`.
    ("FormattedString", "v8ui:FormattedString"),
    // EDT `Picture` ↔ Designer `v8ui:Picture` (префикс `v8ui:` — UI-namespace,
    // объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns; тот же паттерн, что `FormattedString`).
    // Тип «Картинка», встречается типом реквизитов/параметров форм. Сверено по ERP EDT:
    // `<valueType><types>Picture</types>` (exts/ЭкстракторДанных1СВBI/DataProcessors/
    // Экс_ПомощникНастройки|Экс_ГенераторРасширенияПодписок/Forms/Форма/Form.form) ↔ Designer
    // `<v8:Type>v8ui:Picture</v8:Type>` (ChartsOfCalculationTypes/Начисления/Forms/
    // ФормаВидаРасчета/Ext/Form.xml, `xmlns:v8ui` в корне-envelope). Хостится `<v8:Type>`
    // (is_type_set=false — `Picture` не ref-kind → TypeSet-carve-out не нужен).
    ("Picture", "v8ui:Picture"),
    // EDT `Font` ↔ Designer `v8ui:Font` (префикс `v8ui:` — UI-namespace `http://v8.1c.ru/8.1/data/ui`,
    // объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns; ТОТ ЖЕ паттерн, что `Picture`/`FormattedString`).
    // Тип «Шрифт», встречается value-type'ом реквизитов/параметров форм (`<valueType>`) И
    // дискриминатором StyleItem'ов (`<type>Font</type>`). Всегда bare-токен `Font`, без префикса в EDT.
    // Сверено по ERP+SSL: EDT `<valueType><types>Font</types>` (CommonForms/НастройкаКолонтитулов
    // Form.form attr ШрифтСверху/ШрифтСнизу; CommonForms/ПанельОтчетов, РедактированиеТабличногоДокумента)
    // и `<type>Font</type>` (StyleItems/ВажнаяНадписьШрифт .mdo + SSL StyleItems) ↔ Designer
    // `<v8:Type>v8ui:Font</v8:Type>` (designer_8.3.27 CommonForms/НастройкаКолонтитулов/Ext/Form.xml,
    // `xmlns:v8ui` в корне-envelope). Хостится `<v8:Type>` (is_type_set=false — value-type, не
    // ref-kind → TypeSet-carve-out не нужен).
    ("Font", "v8ui:Font"),
    // EDT `VerticalAlign` ↔ Designer `v8ui:VerticalAlign` (префикс `v8ui:` — UI-namespace
    // `http://v8.1c.ru/8.1/data/ui`, объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns; ТОТ ЖЕ
    // паттерн, что `Font`/`Picture`). Тип-перечисление «вертикальное положение», встречается
    // value-type'ом реквизитов форм (`<valueType>`). Сверено по SSL: EDT
    // `<valueType><types>VerticalAlign</types>` (CommonForms/НастройкаКолонтитулов Form.form attrs
    // ВертикальноеПоложениеСверху/Снизу; ПанельОтчетов, ФормаНастроекОтчета, ФормаПоиска) ↔ Designer
    // `<v8:Type>v8ui:VerticalAlign</v8:Type>` (designer_8.5.1 CommonForms/НастройкаКолонтитулов/Ext/
    // Form.xml, `xmlns:v8ui` в корне-envelope). Хостится `<v8:Type>` (is_type_set=false — value-type,
    // не ref-kind → carve-out НЕ нужен; envelope-declared → НЕ в INLINE_NS_QNAMES).
    ("VerticalAlign", "v8ui:VerticalAlign"),
    // EDT `HorizontalAlign` ↔ Designer `v8ui:HorizontalAlign` (префикс `v8ui:` — UI-namespace
    // `http://v8.1c.ru/8.1/data/ui`, объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns; ТОТ ЖЕ
    // паттерн, что sibling `VerticalAlign`/`Font`/`Color`). Тип-перечисление «горизонтальное
    // положение», встречается value-type'ом реквизита формы (`<valueType>`). Сверено по ERP:
    // EDT `<valueType><types>HorizontalAlign</types>` (Catalogs/ЭлементыФинансовыхОтчетов/Forms/
    // РедактированиеЭлементаУсловногоОформления Form.form реквизит «Горизонтальное положение»)
    // ↔ Designer `<v8:Type>v8ui:HorizontalAlign</v8:Type>` (designer_8.3.27 тот же реквизит,
    // `xmlns:v8ui` в корне-envelope). Хостится `<v8:Type>` (is_type_set=false — value-type, не
    // ref-kind → carve-out НЕ нужен; envelope-declared → НЕ в INLINE_NS_QNAMES). NB: НЕ путать с
    // DCS-хостом `<dcscor:value xsi:type="v8ui:HorizontalAlign">` (ДРУГОЙ кодек, DcsCorValue).
    ("HorizontalAlign", "v8ui:HorizontalAlign"),
    // EDT `Color` ↔ Designer `v8ui:Color` (префикс `v8ui:` — UI-namespace, объявлен в ENVELOPE
    // корня формы, БЕЗ инлайн-ns; ТОТ ЖЕ паттерн, что `Font`/`VerticalAlign`). Тип «Цвет»,
    // встречается value-type'ом реквизитов форм (`<valueType>`). НЕ путать с `core:ColorRef`
    // (`<textColor xsi:type="core:ColorRef">` — ДРУГОЙ, ссылочный host, вне alias-таблицы). Сверено
    // по SSL: EDT `<valueType><types>Color</types>` (CommonForms/ПанельОтчетов, ФормаНастроекОтчета,
    // ФормаПоиска, РедактированиеТабличногоДокумента) ↔ Designer `<v8:Type>v8ui:Color</v8:Type>`
    // (designer_8.5.1 CommonForms/ПанельОтчетов/Ext/Form.xml). Хостится `<v8:Type>` (is_type_set=false).
    ("Color", "v8ui:Color"),
    // EDT `SpreadsheetDocument` ↔ Designer `mxl:SpreadsheetDocument` (префикс `mxl:`, local-name
    // СОВПАДАЕТ, но host несёт ИНЛАЙН-объявление `xmlns:mxl="http://v8.1c.ru/8.2/data/spreadsheet"`
    // на самом `<v8:Type>` — тот же паттерн, что `dcsset:SettingsComposer`/`d7p1:Chart`, см.
    // [`INLINE_NS_QNAMES`]/[`inline_ns_for_qname`]; поэтому в PLATFORM_ALIASES, а НЕ в PLATFORM_BARE).
    // Тип «Табличный документ», встречается типом реквизитов форм (backing SpreadsheetDocumentField).
    // Сверено по SSL/ERP: EDT `<valueType><types>SpreadsheetDocument</types>` (SSL edt CommonForms/
    // ОписаниеИзмененийПрограммы + 29 др. форм) ↔ Designer `<v8:Type xmlns:mxl=…>mxl:SpreadsheetDocument`
    // (SSL designer_8.5.1 Catalogs/ДополнительныеОтчетыИОбработки/Forms/ФормаЭлемента/Ext/Form.xml;
    // ERP designer_8.3.27 CommonForms/АЛКОФормаСообщенийОбОшибках/Ext/Form.xml — оба несут инлайн-
    // `xmlns:mxl`). Хостится `<v8:Type>` (is_type_set=false — не ref-kind → carve-out НЕ нужен).
    ("SpreadsheetDocument", "mxl:SpreadsheetDocument"),
    // EDT `FormattedDocument` ↔ Designer `fd:FormattedDocument` (префикс `fd:`, local-name
    // СОВПАДАЕТ, но host несёт ИНЛАЙН-объявление `xmlns:fd="http://v8.1c.ru/8.2/data/formatted-document"`
    // на самом `<v8:Type>` — тот же паттерн, что `mxl:SpreadsheetDocument`/`d7p1:Chart`, см.
    // [`INLINE_NS_QNAMES`]/[`inline_ns_for_qname`]; поэтому в PLATFORM_ALIASES, а НЕ в PLATFORM_BARE).
    // Тип «Форматированный документ», встречается типом реквизитов форм (backing FormattedDocumentField).
    // Сверено по ERP/SSL (55/55 byte-identical, ни одного `<v8:TypeSet>`/не-инлайн-варианта): EDT
    // `<valueType><types>FormattedDocument</types>` (ERP edt CommonForms/ВосстановлениеПаролей + 16 др.
    // форм) ↔ Designer `<v8:Type xmlns:fd="http://v8.1c.ru/8.2/data/formatted-document">fd:FormattedDocument`
    // (ERP designer_8.3.27 CommonForms/ВосстановлениеПаролей/Ext/Form.xml). Хостится `<v8:Type>`
    // (is_type_set=false — value-type, не ref-kind → carve-out НЕ нужен).
    ("FormattedDocument", "fd:FormattedDocument"),
    // EDT `TextDocument` ↔ Designer `d5p1:TextDocument` (префикс `d5p1:`, local-name СОВПАДАЕТ,
    // но host несёт ИНЛАЙН-объявление `xmlns:d5p1="http://v8.1c.ru/8.1/data/txtedt"` на самом
    // `<v8:Type>` — тот же паттерн, что `mxl:SpreadsheetDocument`/`fd:FormattedDocument`/`d7p1:Chart`,
    // см. [`INLINE_NS_QNAMES`]/[`inline_ns_for_qname`]; поэтому в PLATFORM_ALIASES, а НЕ в PLATFORM_BARE).
    // Префикс `d5p1` — авто-генерируемый 1С, но во ВСЕХ 4 вхождениях SSL-среза именно он → byte-exact.
    // Тип «Текстовый документ», встречается value-type'ом реквизитов форм (backing TextDocumentField).
    // Сверено по SSL (4/4 byte-identical): EDT `<valueType><types>TextDocument</types>` (Catalogs/
    // ШаблоныСообщений/Forms/ФормаЭлемента Form.form:3302; СформироватьСообщение; DataProcessors/
    // РаботаСФайлами/Forms/РедактированиеТекстовогоФайла; DocumentJournals/Взаимодействия/Forms/
    // ПараметрыЭлектронногоПисьма) ↔ Designer `<v8:Type xmlns:d5p1="http://v8.1c.ru/8.1/data/txtedt">
    // d5p1:TextDocument</v8:Type>` (designer_8.5.1 Catalogs/ШаблоныСообщений/Forms/ФормаЭлемента/Ext/
    // Form.xml:1442 + 3 др.). Хостится `<v8:Type>` (is_type_set=false — value-type, не ref-kind →
    // carve-out НЕ нужен). NB: НЕ путать с `<templateType>TextDocument</templateType>` (перечисление
    // вида макета — ДРУГОЙ кодек, не Type).
    ("TextDocument", "d5p1:TextDocument"),
    // EDT `DataCompositionSortDirection` ↔ Designer `dcscor:DataCompositionSortDirection` (префикс
    // `dcscor:`, local-name СОВПАДАЕТ — только префикс иной → PLATFORM_ALIASES, не PLATFORM_BARE).
    // ns `dcscor="http://v8.1c.ru/8.1/data-composition-system/core"` объявлен в ENVELOPE корня формы,
    // БЕЗ инлайн-ns на `<v8:Type>` (тот же паттерн, что `v8ui:Font`/`cfg:ConstantsSet`; НЕ как
    // sibling `dcsset:SettingsComposer`, который инлайн — это ДРУГОЙ ns `.../settings` и другой хост).
    // Тип-перечисление СКД «направление сортировки», встречается value-type'ом колонки формы
    // (`<valueType>`/`<Type>`). Сверено по SSL+ERP (byte-identical оба): EDT
    // `<valueType><types>DataCompositionSortDirection</types>` (SSL edt CommonForms/ФормаНастроекОтчета
    // Form.form:10364; ERP edt CommonForms/ФормаНастроекОтчета Form.form:12198) ↔ Designer
    // `<v8:Type>dcscor:DataCompositionSortDirection</v8:Type>` (SSL designer_8.5.1 CommonForms/
    // ФормаНастроекОтчета/Ext/Form.xml:4345, `xmlns:dcscor` в корне-envelope line 2; ERP designer_8.3.27
    // тот же файл:5724). Хостится `<v8:Type>` (is_type_set=false — value-type, не ref-kind → carve-out
    // НЕ нужен; envelope-declared → НЕ в INLINE_NS_QNAMES).
    (
        "DataCompositionSortDirection",
        "dcscor:DataCompositionSortDirection",
    ),
    // EDT `DataCompositionFieldPlacement` ↔ Designer `dcsset:DataCompositionFieldPlacement` (префикс
    // `dcsset:`, local-name СОВПАДАЕТ — только префикс иной → PLATFORM_ALIASES). ВАЖНО: в ФОРМАХ
    // `dcsset` объявлен в ENVELOPE корня (line-2 `xmlns:dcsset`), поэтому `<v8:Type>` НЕСЁТ его БЕЗ
    // инлайн-ns — как `dcscor:DataCompositionSortDirection`, и в ОТЛИЧИЕ от sibling
    // `dcsset:SettingsComposer` (тот инлайн ТОЛЬКО в DataProcessor-ДЕСКРИПТОРАХ, где envelope его
    // не объявляет → INLINE_NS_QNAMES; в форме SettingsComposer тоже envelope-declared). Тип-
    // перечисление СКД «размещение поля», value-type реквизита формы. Сверено по SSL: EDT
    // `<valueType><types>DataCompositionFieldPlacement</types>` (CommonForms/ФормаНастроекОтчета,
    // ФормаПоиска) ↔ Designer `<v8:Type>dcsset:DataCompositionFieldPlacement</v8:Type>` (designer_8.5.1
    // ФормаНастроекОтчета/Ext/Form.xml:4430, `xmlns:dcsset` в envelope). Хостится `<v8:Type>`
    // (is_type_set=false; envelope-declared → НЕ в INLINE_NS_QNAMES).
    (
        "DataCompositionFieldPlacement",
        "dcsset:DataCompositionFieldPlacement",
    ),
    // EDT `DataCompositionGroupType` ↔ Designer `dcscor:DataCompositionGroupType` (префикс `dcscor:`,
    // local-name СОВПАДАЕТ; `dcscor` объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns — как sibling
    // `dcscor:DataCompositionSortDirection`). Тип-перечисление СКД «тип группировки», value-type
    // реквизита формы. Сверено по SSL: EDT `<valueType><types>DataCompositionGroupType</types>`
    // (CommonForms/ФормаНастроекОтчета, ФормаПоиска) ↔ Designer
    // `<v8:Type>dcscor:DataCompositionGroupType</v8:Type>`. Хостится `<v8:Type>` (is_type_set=false).
    (
        "DataCompositionGroupType",
        "dcscor:DataCompositionGroupType",
    ),
    // EDT `DataCompositionPeriodAdditionType` ↔ Designer `dcscor:DataCompositionPeriodAdditionType`
    // (dcscor, local-name СОВПАДАЕТ, envelope-declared — как sibling dcscor-типы). value-type
    // реквизита DCS-формы. SSL: CommonForms/ФормаНастроекОтчета, ФормаПоиска.
    (
        "DataCompositionPeriodAdditionType",
        "dcscor:DataCompositionPeriodAdditionType",
    ),
    // EDT `DataCompositionComparisonType` ↔ Designer `dcsset:DataCompositionComparisonType`
    // (dcsset, local-name СОВПАДАЕТ, envelope-declared в форме — как DataCompositionFieldPlacement,
    // НЕ inline). value-type реквизита DCS-формы. SSL: ФормаНастроекОтчета, ФормаПоиска.
    (
        "DataCompositionComparisonType",
        "dcsset:DataCompositionComparisonType",
    ),
    // EDT `DataCompositionField` ↔ Designer `dcscor:Field` (local-name РАЗНЫЙ! — как
    // `ValueList`↔`v8:ValueListType`; `dcscor` envelope-declared). value-type реквизита DCS-формы.
    // Сверено по SSL: EDT `<types>DataCompositionField</types>` ↔ Designer `<v8:Type>dcscor:Field`
    // (ФормаНастроекОтчета, ФормаПоиска). Хостится `<v8:Type>` (is_type_set=false).
    ("DataCompositionField", "dcscor:Field"),
    // EDT `DataCompositionFilter` ↔ Designer `dcsset:Filter` (local-name РАЗНЫЙ! — как
    // `DataCompositionField`↔`dcscor:Field`, но префикс `dcsset:` — ns `.../settings`).
    // `dcsset` envelope-declared в ФОРМЕ (line-2 `xmlns:dcsset`), поэтому `<v8:Type>` несёт
    // его БЕЗ инлайн-ns — как sibling `dcsset:DataCompositionFieldPlacement`/
    // `dcsset:DataCompositionComparisonType` (НЕ как `dcsset:SettingsComposer`, инлайн только в
    // DataProcessor-дескрипторах). Тип «Отбор компоновки данных», value-type колонки формы.
    // Сверено по SSL: EDT `<valueType><types>DataCompositionFilter</types>` (DataProcessors/
    // ИнтерактивноеИзменениеВыгрузки/Forms/СоставВыгрузки/Form.form:422) ↔ Designer
    // `<v8:Type>dcsset:Filter</v8:Type>` (…/Ext/Form.xml:182, `xmlns:dcsset` в envelope).
    // Хостится `<v8:Type>` (is_type_set=false — value-type; envelope-declared → НЕ в INLINE_NS_QNAMES).
    ("DataCompositionFilter", "dcsset:Filter"),
    // EDT `DynamicList` ↔ Designer `cfg:DynamicList` (префикс `cfg:`, local-name СОВПАДАЕТ —
    // только префикс иной → PLATFORM_ALIASES, не PLATFORM_BARE; тот же паттерн, что
    // `cfg:ConstantsSet`/`cfg:ReportBuilder`). ns `cfg="http://v8.1c.ru/8.1/data/enterprise/
    // current-config"` объявлен в ENVELOPE корня формы, БЕЗ инлайн-ns на `<v8:Type>` (envelope-
    // declared → НЕ в INLINE_NS_QNAMES). Тип «Динамический список», встречается value-type'ом
    // ДАННЫХ-реквизита формы (`<valueType><types>DynamicList</types>`; backing-тип реквизита
    // Таблицы-динамического-списка). Сверено по SSL (byte-identical): EDT
    // `<valueType><types>DynamicList</types>` (CommonForms/ВыборКонтакта Form.form:1541) ↔ Designer
    // `<v8:Type>cfg:DynamicList</v8:Type>` (designer_8.5.1 CommonForms/ВыборКонтакта/Ext/Form.xml:715,
    // `xmlns:cfg` в корне-envelope line 3). Хостится `<v8:Type>` (is_type_set=false — value-type,
    // НЕ ref-kind: `DynamicList` без `.Имя` не в REF_KINDS → carve-out НЕ нужен).
    ("DynamicList", "cfg:DynamicList"),
    // EDT `PDFDocument` ↔ Designer `pdfdoc:PDFDocument` (ФИКС-префикс `pdfdoc:`, local-name
    // СОВПАДАЕТ, но host несёт ИНЛАЙН-объявление `xmlns:pdfdoc="http://v8.1c.ru/8.3/data/pdf"`
    // на самом `<v8:Type>` — тот же паттерн, что `mxl:SpreadsheetDocument`/`fd:FormattedDocument`,
    // см. [`INLINE_NS_QNAMES`]/[`inline_ns_for_qname`]; поэтому в PLATFORM_ALIASES + INLINE_NS_QNAMES).
    // Тип «Документ PDF», value-type реквизита формы (backing PDFDocumentField). `pdfdoc` НЕ в
    // [`super::form::DESIGNER_FORM_NS`]/`_ERP` → инлайн ОБЯЗАТЕЛЕН (сверено: 6/6 designer с инлайном).
    // EDT `<valueType><types>PDFDocument</types>` (ERP edt DataProcessors/СервисДоставки/Forms/
    // ПросмотрPDF/Form.form:135) ↔ Designer `<v8:Type xmlns:pdfdoc="http://v8.1c.ru/8.3/data/pdf">
    // pdfdoc:PDFDocument</v8:Type>` (designer_8.3.27 …/ПросмотрPDF/Ext/Form.xml:54). Хостится
    // `<v8:Type>` (is_type_set=false — value-type, не ref-kind → carve-out НЕ нужен).
    ("PDFDocument", "pdfdoc:PDFDocument"),
    // EDT `AccountingRecordType` ↔ Designer `ent:AccountingRecordType` (ФИКС-префикс `ent:` —
    // enterprise-ns `http://v8.1c.ru/8.1/data/enterprise`, объявлен в ENVELOPE корня формы, БЕЗ
    // инлайн-ns; local-name СОВПАДАЕТ — только префикс иной → PLATFORM_ALIASES, НЕ INLINE_NS_QNAMES;
    // тот же паттерн envelope-declared, что `dcscor:*`/`cfg:*`). `ent` объявлен в
    // [`super::form::DESIGNER_FORM_NS`]/`_ERP` → инлайна НЕТ. Тип-перечисление «вид движения
    // бухгалтерской записи» (Дебет/Кредит), value-type реквизита формы. Сверено по ERP (8×
    // designer, ВСЕ без инлайна): EDT `<valueType><types>AccountingRecordType</types>` (edt
    // Documents/ОперацияМеждународный/Forms/ФормаДокумента/Form.form:6378) ↔ Designer
    // `<v8:Type>ent:AccountingRecordType</v8:Type>` (designer_8.3.27 …/Ext/Form.xml:2646). Хостится
    // `<v8:Type>` (is_type_set=false). NB: sibling `ent:AccountType` (вид счёта) живёт в
    // form-value-кодеке (`ir::value::PropertyValue::AccountType`) — ДРУГОЙ host, не Type-набор.
    ("AccountingRecordType", "ent:AccountingRecordType"),
];

/// QName Designer-типов, которые хостятся `<v8:Type>` с ИНЛАЙН-объявлением ns на самом
/// элементе. `(qname, (attr_name, uri))`. На чтении — claim этого атрибута; на записи —
/// эмиссия его обратно (byte-exact). `cfg:`-типы тут НЕ нужны (`cfg` в envelope корня).
const INLINE_NS_QNAMES: &[(&str, (&str, &str))] = &[
    (
        "dcsset:SettingsComposer",
        (
            "xmlns:dcsset",
            "http://v8.1c.ru/8.1/data-composition-system/settings",
        ),
    ),
    (
        "mxl:SpreadsheetDocument",
        ("xmlns:mxl", "http://v8.1c.ru/8.2/data/spreadsheet"),
    ),
    (
        "fd:FormattedDocument",
        ("xmlns:fd", "http://v8.1c.ru/8.2/data/formatted-document"),
    ),
    (
        "d5p1:TextDocument",
        ("xmlns:d5p1", "http://v8.1c.ru/8.1/data/txtedt"),
    ),
    (
        "pdfdoc:PDFDocument",
        ("xmlns:pdfdoc", "http://v8.1c.ru/8.3/data/pdf"),
    ),
];

/// Инлайн-ns-объявление для Designer QName, если этот тип его требует.
pub(crate) fn inline_ns_for_qname(qname: &str) -> Option<(&'static str, &'static str)> {
    INLINE_NS_QNAMES
        .iter()
        .find(|(q, _)| *q == qname)
        .map(|(_, ns)| *ns)
}

/// Типы с АВТО-ГЕНЕРИРУЕМЫМ Designer-префиксом `d{N}p1` (N = 1-based ГЛУБИНА host-элемента
/// `<v8:Type>` в документе; сериализатор 1С нумерует авто-префиксы позицией). Инлайн-ns
/// объявляется ВСЕГДА (эти uri не входят ни в один envelope). `(canon, designer-local, uri)`.
///
/// Витнессы: дескриптор ОценкаПроизводительности `d7p1:Chart` (MetaDataObject>DataProcessor>
/// ChildObjects>Attribute>Properties>Type>v8:Type = 7); формы ПодборЦелевогоВремени
/// `d5p1:Chart` и КартаМаршрутаБизнесПроцесса `d5p1:FlowchartContextType`
/// (Form>Attributes>Attribute>Type>v8:Type = 5). EDT-канон сверен твинами:
/// `<types>Chart</types>` / `<types>GraphicalSchema</types>`.
pub(crate) const AUTO_NS_TYPES: &[(&str, &str, &str)] = &[
    (
        "ConditionalAppearance",
        "ConditionalAppearance",
        "http://v8.1c.ru/8.3/data/entext",
    ),
    ("Chart", "Chart", "http://v8.1c.ru/8.2/data/chart"),
    // «Диаграмма Ганта» — value-type реквизита формы (backing GanttChartField). ТОТ ЖЕ
    // авто-NS-паттерн и ТА ЖЕ uri, что `Chart` (`http://v8.1c.ru/8.2/data/chart`), иной лишь
    // local-name. Префикс `d{N}p1` авто-генерируемый; ERP-корпус даёт ТОЛЬКО глубину 5
    // (форма): 17/17 `<v8:Type xmlns:d5p1="http://v8.1c.ru/8.2/data/chart">d5p1:GanttChart`,
    // ни одного descriptor-`d7p1` (GanttChartField живёт в ФОРМЕ — как GeographicalSchema; в
    // дескрипторе тип не встречается, поэтому writer'а с пустым envelope сюда не приводят).
    // EDT-твин сверен на ОДНОЙ форме (DataProcessor ДиспетчированиеПроизводства):
    // `<valueType><types>GanttChart</types>` (Form.form:14819) ↔ Designer
    // `d5p1:GanttChart` (…/Ext/Form.xml:6840). Хостится `<v8:Type>` (is_type_set=false).
    ("GanttChart", "GanttChart", "http://v8.1c.ru/8.2/data/chart"),
    (
        "GraphicalSchema",
        "FlowchartContextType",
        "http://v8.1c.ru/8.2/data/graphscheme",
    ),
    // «Географическая схема» — value-type реквизита формы. ERP-witness БизнесРегионы.
    // ФормаВыбораГеографическогоРегиона: Designer `<v8:Type xmlns:d5p1="http://v8.1c.ru/8.2/
    // data/geo">d5p1:GeographicalSchema</v8:Type>` (глубина 5 — форма) ↔ EDT твин
    // `<types>GeographicalSchema</types>` (Form.form:258).
    (
        "GeographicalSchema",
        "GeographicalSchema",
        "http://v8.1c.ru/8.2/data/geo",
    ),
    // «Единица интервала времени анализа данных» — value-type реквизита формы (периодичность
    // Gantt-каскада). ТОТ ЖЕ авто-NS-паттерн (`d{N}p1`, инлайн-ns), иная uri/domain
    // (data-analysis). ERP-witness DataProcessor ДиспетчированиеПроизводстваПооперационное.
    // ДиспетчированиеПроизводства (реквизит Периодичность, id 20): EDT
    // `<valueType><types>DataAnalysisTimeIntervalUnitType</types>` (Form.form:10429) ↔ Designer
    // `<v8:Type xmlns:d5p1="http://v8.1c.ru/8.2/data/data-analysis">
    // d5p1:DataAnalysisTimeIntervalUnitType</v8:Type>` (…/Ext/Form.xml:5239, глубина 5 — форма).
    // local-name СОВПАДАЕТ (edt bare == designer local). Хостится `<v8:Type>` (is_type_set=false).
    (
        "DataAnalysisTimeIntervalUnitType",
        "DataAnalysisTimeIntervalUnitType",
        "http://v8.1c.ru/8.2/data/data-analysis",
    ),
];

/// Разобрать QName авто-префиксного типа: `d{N}p1:{local}` → `(canon, prefix)`.
/// НЕ проверяет uri (её сверяет вызывающий по inline-атрибуту хоста).
pub(crate) fn auto_ns_qname_to_canon(qname: &str) -> Option<(&'static str, &'static str, &str)> {
    let (prefix, local) = qname.split_once(':')?;
    let digits = prefix.strip_prefix('d')?.strip_suffix("p1")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    AUTO_NS_TYPES
        .iter()
        .find(|(_, l, _)| *l == local)
        .map(|(canon, _, uri)| (*canon, *uri, prefix))
}

/// URI пространства имён ССЫЛОЧНЫХ типов ТЕКУЩЕЙ конфигурации. Designer-ДЕСКРИПТОРЫ и
/// ФОРМЫ объявляют её префиксом `cfg` в корне-envelope, поэтому ref-тип пишется
/// `cfg:CatalogRef.Имя` БЕЗ инлайн-объявления. Документы, чей корень `cfg` НЕ объявляет
/// (Designer-сайдкар `Ext/Predefined.xml`), получают АВТО-префикс `d{N}p1` + инлайн-ns на
/// самом `<v8:Type>` — см. [`encode_scoped_auto_cfg`] / [`auto_cfg_qname_to_canon`].
pub const CURRENT_CONFIG_NS: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";

/// Разобрать QName АВТО-префиксного ССЫЛОЧНОГО типа текущей конфигурации:
/// `d{N}p1:{CatalogRef.Имя}` → `(canon_bare, prefix)`. Отличается от
/// [`auto_ns_qname_to_canon`] тем, что local — не фиксированный тип из [`AUTO_NS_TYPES`], а
/// ЛЮБОЙ known-ref (тот же домен, что ветка `cfg:` в [`qname_to_canon`]). uri сверяет
/// вызывающий по инлайн-атрибуту хоста (§1.0 — иной uri = другой тип).
pub(crate) fn auto_cfg_qname_to_canon(qname: &str) -> Option<(String, &str)> {
    let (prefix, local) = qname.split_once(':')?;
    let digits = prefix.strip_prefix('d')?.strip_suffix("p1")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !is_known_ref(local) {
        return None;
    }
    Some((local.to_string(), prefix))
}

/// Платформенные типы с СОВПАДАЮЩИМ local-name (Designer = `v8:` + canon). Замкнутый
/// allow-list (§1.0: неизвестный платформенный id → ошибка, не guess). Покрывает
/// SSL-`.mdo`-срез (агрегатный скан всего EDT-дерева).
const PLATFORM_BARE: &[&str] = &[
    "ValueStorage",
    "FixedStructure",
    "FixedArray",
    "FixedMap",
    "UUID",
    "ValueTable",
    "ValueTree",
    "StandardPeriod",
    // EDT bare `StandardBeginningDate` ↔ Designer `v8:StandardBeginningDate` (механический
    // `v8:` + тот же local-name; ns `http://v8.1c.ru/8.1/data/core`). Тип «стандартная дата
    // начала». Сверено по ERP: EDT `<types>StandardBeginningDate</types>` (реквизит
    // `ДатаОтгрузки` DataProcessor `ПолучениеИВозвратМатериалов`) ↔ Designer
    // `<v8:Type>v8:StandardBeginningDate</v8:Type>` (17 вхождений корпуса). Родственный
    // `v8:StandardBeginningDateVariant` — НЕ тип-ссылка, а xsi-тип ЗНАЧЕНИЯ внутри
    // DCS-настроек (variant-ячейка) — живёт в generic-DCS-ресериализаторе, не здесь.
    "StandardBeginningDate",
    // EDT bare `TypeDescription` ↔ Designer `v8:TypeDescription` (механический `v8:` +
    // тот же local-name; ns `http://v8.1c.ru/8.1/data/core`). Тип «описание типов»,
    // встречается в параметрах/реквизитах форм. Сверено по SSL: EDT `<types>TypeDescription`
    // (CommonForm.ВыборРолиИсполнителя/РедактированиеГиперссылки, Documents/*/ФормаДокумента)
    // ↔ Designer `<v8:Type>v8:TypeDescription` (ВыборРолиИсполнителя/Ext/Form.xml). Хостится
    // `<v8:Type>` (is_type_set=false).
    "TypeDescription",
    // EDT bare `Null` ↔ Designer `v8:Null` (механический `v8:` + тот же local-name; ns
    // `http://v8.1c.ru/8.1/data/core`). Тип «Null» (пустое значение), встречается в составных
    // value-type'ах реквизитов DCS-форм. Сверено по SSL: EDT `<types>Null</types>` ↔ Designer
    // `<v8:Type>v8:Null</v8:Type>` (CommonForms/ФормаНастроекОтчета, ФормаПоиска).
    "Null",
    // EDT bare `FillChecking` ↔ Designer `v8:FillChecking` (механический `v8:` + тот же
    // local-name; ns `http://v8.1c.ru/8.1/data/core`). Тип-перечисление «проверка заполнения»
    // (ПроверкаЗаполнения), встречается value-type'ом колонки формы. Сверено по SSL: EDT
    // `<valueType><types>FillChecking</types>` (DataProcessors/СкрытиеКонфиденциальнойИнформации/
    // Forms/Форма/Form.form:2319) ↔ Designer `<v8:Type>v8:FillChecking</v8:Type>` (…/Ext/Form.xml:1167).
    // Хостится `<v8:Type>` (is_type_set=false).
    "FillChecking",
    // EDT bare `Type` ↔ Designer `v8:Type` (тип «Тип» — мета-тип «значение ЕСТЬ Тип/
    // ОписаниеТипов»; механический `v8:` + тот же local-name; ns `http://v8.1c.ru/8.1/data/core`,
    // envelope-declared → БЕЗ инлайна). КРИТИЧНО: это ЗНАЧЕНИЕ-тип — ТЕКСТ компоненты `v8:Type`,
    // а НЕ служебный host-тег `<v8:Type>` (тот оборачивает ЛЮБУЮ компоненту и различается по
    // ИМЕНИ тега prefix=v8/local=Type, независимо от текста; добавление сюда трогает лишь
    // канонизацию ТЕКСТА "v8:Type" в `qname_to_canon`/`canon_to_qname`, НЕ разбор host-тега).
    // Сверено по ERP (21× designer `<v8:Type>v8:Type</v8:Type>`): EDT
    // `<valueType><types>Type</types>` (edt Catalogs/КлючиРеестраДокументов/Forms/ФормаВыбора/
    // Form.form:604) ↔ Designer `<v8:Type>v8:Type</v8:Type>` (designer_8.3.27 …/Ext/Form.xml:244).
    // Хостится `<v8:Type>` (is_type_set=false).
    "Type",
    // `ConstantsSet`/`Chart`/`ReportBuilder`/`DataCompositionSettingsComposer` — НЕ
    // механический `v8:`-bare: их Designer-QName (`cfg:ConstantsSet`/`d7p1:Chart`/
    // `cfg:ReportBuilder`/`dcsset:SettingsComposer`) — иной префикс (и иногда local-name/
    // инлайн-ns) → живут в [`PLATFORM_ALIASES`].
];

/// Префиксы Designer-ссылочных типов (`cfg:` + bare canon = `cfg:CatalogRef.X`).
/// Замкнутый allow-list local-name-«основ» (§1.0). `CatalogRef.Имя` каноничен bare.
///
/// `*Object`/`*Manager`/`*RecordSet` — тоже `cfg:`-адресуемые типы прикладных объектов
/// (DefinedType ссылается на них наряду с `*Ref`); присутствие в SSL-корпусе
/// DefinedTypes подтверждено сканом (edt bare == designer `cfg:` для каждого).
const REF_KINDS: &[&str] = &[
    "AnyRef",
    "CatalogRef",
    "DocumentRef",
    "EnumRef",
    "ChartOfCharacteristicTypesRef",
    "ChartOfAccountsRef",
    "ChartOfCalculationTypesRef",
    "BusinessProcessRef",
    "TaskRef",
    "ExchangePlanRef",
    "DefinedType",
    "CatalogObject",
    "DocumentObject",
    "InformationRegisterRecordSet",
    // `InformationRegisterRecordManager.Имя` — cfg-адресуемый МЕНЕДЖЕР-тип записи регистра
    // сведений (ссылка на набор-запись КОНКРЕТНОГО регистра; аналог `*RecordSet`, но category
    // `RecordManager` в GeneratedType). Встречается concrete-формой value-type'ом реквизита
    // формы записи регистра (`<v8:Type>cfg:InformationRegisterRecordManager.<Имя>`). Сверено по
    // SSL (43 EDT формы == 43 Designer формы): EDT `<types>InformationRegisterRecordManager.
    // ХранилищеФайлов</types>` (InformationRegisters/ХранилищеФайлов/Forms/ФормаЗаписи/Form.form:140)
    // ↔ Designer `<v8:Type>cfg:InformationRegisterRecordManager.ХранилищеФайлов</v8:Type>`
    // (…/Ext/Form.xml:43). Concrete `Kind.Имя` → `<v8:Type>` (is_type_set=false). Только
    // ConcreteForm в корпусе (bare не встречен); классифицируется как sibling `*RecordSet`.
    "InformationRegisterRecordManager",
    // Дополнительно встречены в DefinedTypes-корпусе (object/manager-виды как cfg-ссылки):
    "BusinessProcessObject",
    "ChartOfCharacteristicTypesObject",
    "TaskObject",
    "ConstantValueManager",
    // Встречены в EventSubscription-корпусе `Source` (object/manager/record-set-виды как
    // cfg-ссылки; bare → `<v8:TypeSet>`, `Kind.Имя` → `<v8:Type>` — то же правило, что у
    // прочих ref-kind'ов). Скан всего SSL-корпуса: ни один из них НЕ нарушает is_type_set.
    // NB: bare-`*Manager` в SSL-Designer ОТСУТСТВУЮТ (скан: только concrete `Kind.Имя`) —
    // прежняя формулировка «bare всегда TypeSet» была вакуумной для менеджеров; ERP-корпус
    // дал контр-witness (bare `XxxManager` → `<v8:Type>`) — см. [`NON_TYPESET_BARE_REFS`].
    "AccountingRegisterRecordSet",
    "AccumulationRegisterRecordSet",
    "CalculationRegisterRecordSet",
    "ChartOfAccountsObject",
    "ChartOfCalculationTypesObject",
    "ExchangePlanObject",
    "CatalogManager",
    "DocumentManager",
    "BusinessProcessManager",
    // --- МЕНЕДЖЕРНЫЕ семейства из ERP EventSubscription `Source` (полный census
    //     .fixtures/ERP: edt `<types>` ↔ designer `cfg:`-QName, счёты видов совпали 1:1).
    //     Concrete `Kind.Имя` → `<v8:Type>cfg:Kind.Имя`; BARE `Kind` («менеджер любого
    //     объекта вида») → тоже `<v8:Type>` (НЕ TypeSet — см. [`NON_TYPESET_BARE_REFS`]).
    //     cf-сторона: concrete-менеджеры резолвятся writer'ом из IR `generated_types`
    //     (PRODUCED_CATEGORIES вида несут `designer_type_name == Kind`); BARE-менеджеры —
    //     платформенные фикс-guid'ы, в builtin-таблице cf НЕ витнессированы → cf-запись
    //     даёт типизированный отказ (`TypeRegistry::guid_for`, §1.0). ---
    // `InformationRegisterManager.Имя` — 30 concrete (witness: ES
    // ЗаблокироватьОткрытиеФормНаМобильномКлиентеЭДО →
    // `InformationRegisterManager.ПоддерживаемыеФорматыЭлектронныхДокументов`) + 1 bare
    // (ES ТарификацияОбработкаПолученияФормы).
    "InformationRegisterManager",
    // `DataProcessorManager.Имя` — 10 concrete в ERP ES (witness:
    // ЗаблокироватьОткрытиеФормНаМобильномКлиентеЭДО).
    "DataProcessorManager",
    // `ExchangePlanManager.Имя` — 2 concrete в ERP ES (witness: ОбменДаннымиПриСозданииПланаОбмена).
    "ExchangePlanManager",
    // `ReportManager.Имя` — 1 concrete в ERP ES (witness:
    // ЗаблокироватьОткрытиеФормНаМобильномКлиентеЭДО).
    "ReportManager",
    // BARE-only менеджеры ERP ES (по 1 witness'у каждый; все — в
    // ES ТарификацияОбработкаПолученияФормы, edt `<types>Kind</types>` ↔ designer
    // `<v8:Type>cfg:Kind</v8:Type>` byte-parallel):
    "TaskManager",
    "DocumentJournalManager",
    "ChartOfCharacteristicTypesManager",
    "ChartOfCalculationTypesManager",
    "ChartOfAccountsManager",
    "CalculationRegisterManager",
    "AccumulationRegisterManager",
    "AccountingRegisterManager",
    // `SequenceRecordSet` / `RecalculationRecordSet` — bare-only recordset-семейства ERP ES
    // (по 2 witness'а: МиграцияПриложенийПередЗаписьюНабора / СводныеПриложенияПередЗаписьюНабора);
    // designer хостит `<v8:TypeSet>` — как прочие bare `*RecordSet` (правило is_type_set без
    // исключений). cf: платформенные фикс-guid'ы НЕ витнессированы → typed-refusal на записи.
    "SequenceRecordSet",
    "RecalculationRecordSet",
    // `DataProcessorObject.Имя` — cfg-адресуемый тип объекта обработки (полнотекстовый поиск
    // и т.п.), встречается concrete-формой (`cfg:DataProcessorObject.<Имя>`) value-type'ом
    // реквизита формы. SSL: CommonForms/ФормаПоиска (`cfg:DataProcessorObject.Полнотекстовый…`).
    "DataProcessorObject",
    // `ReportObject` — абстрактный объектный тип отчёта. И bare `ReportObject`, и
    // `ReportObject.Имя` — cfg-адресуемы. ОСОБЕННОСТЬ: bare-форма в Designer хостится как
    // `<v8:Type>` (НЕ `<v8:TypeSet>`), в отличие от прочих bare ref-kind'ов → внесён в
    // [`NON_TYPESET_BARE_REFS`]. Сверено по SSL (см. там).
    "ReportObject",
    // `BusinessProcessRoutePointRef.<БП>` — ссылка на точку маршрута бизнес-процесса; тип
    // реквизита «ТочкаМаршрута» табличных частей БП. Сверено по ERP EDT:
    // `<types>BusinessProcessRoutePointRef.СогласованиеПродажи</types>` (СогласованиеПродажи.mdo
    // и 4 др. БП). Concrete `Kind.Имя` → `<v8:Type>cfg:BusinessProcessRoutePointRef.Имя`
    // (механическое `cfg:` — универсальное правило concrete-ref'ов; designer-корпус БП
    // отсутствует, контр-примера нет; producedTypes независимо именует `BusinessProcessRoutePointRef`).
    "BusinessProcessRoutePointRef",
];

/// Ref-kind'ы, у которых BARE-форма в Designer хостится как `<v8:Type>` (а НЕ `<v8:TypeSet>`),
/// ВОПРЕКИ общему правилу «bare ref-kind = type-set». `ReportObject` — абстрактный объектный
/// тип отчёта: и bare `ReportObject`, и `ReportObject.Имя` → `<v8:Type>cfg:ReportObject[.Имя]`.
/// Сверено по SSL: CommonForm.ВспомогательнаяФормаНастроекОтчета bare →
/// `<v8:Type>cfg:ReportObject`; Reports/АнализОпроса/ФормаОтчета concrete →
/// `<v8:Type>cfg:ReportObject.АнализОпроса` (оба `<v8:Type>`, ни разу `<v8:TypeSet>`).
///
/// МЕНЕДЖЕРНЫЕ виды: bare `XxxManager` → `<v8:Type>` витнессировано ERP ES
/// ТарификацияОбработкаПолученияФормы (12 видов, edt `<types>Kind</types>` ↔ designer
/// `<v8:Type>cfg:Kind</v8:Type>` попарно; в designer-ERP НИ ОДНОГО `<v8:TypeSet>cfg:XxxManager`,
/// в SSL bare-менеджеров нет вовсе). ЕДИНСТВЕННОЕ исключение семейства —
/// `ConstantValueManager`: его bare хостится `<v8:TypeSet>` (witnessed 4× ERP + 1× SSL) —
/// поэтому его тут НЕТ. `DataProcessorManager`/`ExchangePlanManager`/`ReportManager` bare в
/// корпусах не встречены (только concrete) — внесены по семейному правилу 12/12 без
/// контр-примера (как `BusinessProcessRoutePointRef`: экстраполяция задокументирована).
const NON_TYPESET_BARE_REFS: &[&str] = &[
    "ReportObject",
    "AccountingRegisterManager",
    "AccumulationRegisterManager",
    "BusinessProcessManager",
    "CalculationRegisterManager",
    "CatalogManager",
    "ChartOfAccountsManager",
    "ChartOfCalculationTypesManager",
    "ChartOfCharacteristicTypesManager",
    "DataProcessorManager",
    "DocumentJournalManager",
    "DocumentManager",
    "ExchangePlanManager",
    "InformationRegisterManager",
    "ReportManager",
    "TaskManager",
];

/// `Characteristic` — особый ССЫЛОЧНЫЙ kind: всегда `Characteristic.Имя`, и в Designer
/// он хостится как `<v8:TypeSet>` (type-set по природе — ссылка резолвится в НАБОР
/// типов характеристики). НЕ в [`REF_KINDS`] (там — обычные `<v8:Type>`-рефы).
const CHARACTERISTIC_KIND: &str = "Characteristic";

/// Канонизировать ССЫЛОЧНЫЙ id (EDT bare уже канон). Возвращает `true`, если id —
/// валидная ссылка из allow-list: `Kind.Имя` (конкретная ссылка), `Kind` (BARE —
/// type-set «любой объект вида», напр. EDT `<types>ExchangePlanRef</types>`), либо
/// `Characteristic.Имя`/`DefinedType.Имя` (type-set-рефы).
pub(crate) fn is_known_ref(canon: &str) -> bool {
    // `AnyRef` — без имени; остальные — `Kind` (bare type-set) или `Kind.Имя`.
    if canon == "AnyRef" {
        return true;
    }
    match canon.split_once('.') {
        // `Kind.Имя`: обычный ref-kind ИЛИ Characteristic (type-set с именем).
        Some((kind, name)) => {
            !name.is_empty() && (REF_KINDS.contains(&kind) || kind == CHARACTERISTIC_KIND)
        }
        // Bare `Kind` (без имени) — type-set «любой объект вида». Только для ref-kinds
        // (не Characteristic: тот всегда с именем). Проверено корпусом: EDT bare
        // `<types>ExchangePlanRef</types>` ↔ Designer `<v8:TypeSet>cfg:ExchangePlanRef`.
        None => REF_KINDS.contains(&canon),
    }
}

/// Является ли канонический id TYPE-SET'ом (Designer хостит его как `<v8:TypeSet>`, а не
/// `<v8:Type>`). Проверено по ВСЕМУ SSL-корпусу: type-set ⇔ BARE ref-kind (без `.Имя`)
/// ЛИБО `DefinedType.Имя`/`Characteristic.Имя` (ссылка резолвится в набор типов).
/// Конкретные `Kind.Имя` (`CatalogRef.Товары` и пр.) — обычный `<v8:Type>`. EDT хостит
/// ОБА вида одинаково (`<types>`), поэтому предикат нужен лишь Designer-проекции.
pub(crate) fn is_type_set(canon: &str) -> bool {
    match canon.split_once('.') {
        Some((kind, _)) => kind == "DefinedType" || kind == CHARACTERISTIC_KIND,
        // Bare ref-kind (`ExchangePlanRef`, `CatalogRef`, …) — type-set; `AnyRef` тоже.
        // Исключение: [`NON_TYPESET_BARE_REFS`] (напр. `ReportObject`) — bare хостится
        // `<v8:Type>`, не `<v8:TypeSet>` (сверено корпусом).
        None => {
            (canon == "AnyRef" || REF_KINDS.contains(&canon))
                && !NON_TYPESET_BARE_REFS.contains(&canon)
        }
    }
}

/// EDT bare id → проверка, что он канонически известен (§1.0: иначе ошибка).
/// Primitive/platform/ref — каждый из своего allow-list.
pub(crate) fn edt_id_known(canon: &str) -> bool {
    PRIMITIVES.iter().any(|(c, _)| *c == canon)
        || PLATFORM_ALIASES.iter().any(|(c, _)| *c == canon)
        || PLATFORM_BARE.contains(&canon)
        || AUTO_NS_TYPES.iter().any(|(c, _, _)| *c == canon)
        // Сырой guid — ссылка на ОпределяемыйТип по containerType-typeId (EDT
        // `<types>{guid}</types>` ⟺ Designer `<v8:TypeId>`; witness Файлы/ДоступныеАнкеты).
        || is_type_uuid(canon)
        || is_known_ref(canon)
}

/// UUID-форма канон-id (36 символов `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`, hex в нижнем
/// регистре — как эмитят оба диалекта).
pub(crate) fn is_type_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit() && !b.is_ascii_uppercase(),
        })
}

/// Designer QName → канонический bare id (§1.2). `None` ⇒ неизвестный QName/алиас
/// (§1.0: вызывающий обязан вернуть ОШИБКУ, не guess).
pub(crate) fn qname_to_canon(qname: &str) -> Option<String> {
    if let Some((canon, _)) = PRIMITIVES.iter().find(|(_, q)| *q == qname) {
        return Some((*canon).to_string());
    }
    if let Some((canon, _)) = PLATFORM_ALIASES.iter().find(|(_, q)| *q == qname) {
        return Some((*canon).to_string());
    }
    if let Some(bare) = qname.strip_prefix("v8:") {
        if PLATFORM_BARE.contains(&bare) {
            return Some(bare.to_string());
        }
        return None;
    }
    if let Some(bare) = qname.strip_prefix("cfg:") {
        if is_known_ref(bare) {
            return Some(bare.to_string());
        }
        return None;
    }
    None
}

/// Канонический bare id → Designer QName (§1.2). `None` ⇒ неизвестный canon id.
pub(crate) fn canon_to_qname(canon: &str) -> Option<String> {
    if let Some((_, q)) = PRIMITIVES.iter().find(|(c, _)| *c == canon) {
        return Some((*q).to_string());
    }
    if let Some((_, q)) = PLATFORM_ALIASES.iter().find(|(c, _)| *c == canon) {
        return Some((*q).to_string());
    }
    if PLATFORM_BARE.contains(&canon) {
        return Some(format!("v8:{canon}"));
    }
    if is_known_ref(canon) {
        return Some(format!("cfg:{canon}"));
    }
    None
}

/// КАНОН-id платформенного типа по DESIGNER local-name'у из `current-config`-QName'а
/// (`cfg:<local>`), когда этот local-name — АЛИАС (отличается от канона). `None` — local-name
/// уже канонический (или не платформенный).
///
/// Нужен потребителям, которые видят СЫРОЙ Designer-QName и резолвят его по канон-реестру, а не
/// через [`decode`]/[`encode`] — прежде всего DCS-телу (`formats_cf::dcs_body`): DCS хранится как
/// generic XML re-serializer, поэтому в IR лежит local-name ИСХОДНОГО диалекта. EDT несёт
/// `cfg:AnyRef`, Designer — `cfg:AnyIBRef`; реестр знает только канон `AnyRef`, поэтому
/// designer→cf отказывал на 6 объектах («canon id "AnyIBRef" has no Ref-type-id GUID»).
///
/// Из [`PLATFORM_ALIASES`] сюда попадает РОВНО один `cfg:`-алиас — `cfg:AnyIBRef` → `AnyRef`
/// (у `cfg:ConstantsSet`/`cfg:ReportBuilder` local-name СОВПАДАЕТ с каноном, у остальных
/// алиасов другой префикс). Таблица одна, дубля витнесса нет.
pub fn canon_for_config_local(local: &str) -> Option<&'static str> {
    PLATFORM_ALIASES.iter().find_map(|(canon, qn)| {
        let alias = qn.strip_prefix("cfg:")?;
        (alias == local && alias != *canon).then_some(*canon)
    })
}
