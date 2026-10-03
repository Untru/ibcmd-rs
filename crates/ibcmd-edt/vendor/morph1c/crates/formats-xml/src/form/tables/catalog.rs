//! Каталоги видов контролов: Addition/Field/Group/Decoration «kind»-строки + аксессоры.

use crate::form::fields::FieldProj;
use crate::form::tables::{
    DesSlot, BUTTON_GROUP_EXT, CALENDAR_FIELD_EXT, CHART_FIELD_EXT, CHECK_BOX_FIELD_EXT,
    COLUMN_GROUP_EXT, COMMAND_BAR_EXT, DES_CALENDAR_ORDER, DES_FIELD_ORDER,
    DES_LABEL_DECORATION_ORDER, DES_PICTURE_DECORATION_ORDER, DES_SPREADSHEET_ORDER,
    FLOWCHART_FIELD_EXT, FORMATTED_FIELD_EXT, GANTT_CHART_FIELD_EXT, HTML_FIELD_EXT,
    IMAGE_FIELD_EXT, INPUT_FIELD_EXT, LABEL_DECORATION_EXT, LABEL_FIELD_EXT, PAGE_EXT, PAGES_EXT,
    PDF_DOCUMENT_FIELD_EXT, PERIOD_FIELD_EXT, PICTURE_DECORATION_EXT, POPUP_EXT,
    PROGRESS_FIELD_EXT, RADIO_BUTTON_FIELD_EXT, SPREADSHEET_FIELD_EXT, TEXT_FIELD_EXT,
    TRACK_BAR_FIELD_EXT, USUAL_GROUP_EXT,
};

// ============================ Table: добавления ============================

/// Каталог-строка вида ДОБАВЛЕНИЯ Таблицы: канонический код (= Designer-тег) + EDT-теги/
/// дискриминатор + extInfo xsi:type + Designer-константа `<AdditionSource><Type>`.
pub(crate) struct AdditionKind {
    /// Канонический код (= Designer-тег элемента).
    pub kind: &'static str,
    /// EDT-тег (`searchStringAddition`/…).
    pub edt_tag: &'static str,
    /// EDT `<type>`-дискриминатор (`None` — searchString, у него `<type>` отсутствует).
    pub edt_type: Option<&'static str>,
    /// EDT `<extInfo xsi:type="…">`.
    pub ext_xsi: &'static str,
    /// Designer-константа `<AdditionSource><Type>` (реконструируется из вида).
    pub des_source_type: &'static str,
}

/// Поддержанные виды добавлений Таблицы (в каноническом порядке эмиссии).
pub(crate) static ADDITION_KINDS: &[AdditionKind] = &[
    AdditionKind {
        kind: "SearchStringAddition",
        edt_tag: "searchStringAddition",
        edt_type: None,
        ext_xsi: "form:SearchStringAdditionExtInfo",
        des_source_type: "SearchStringRepresentation",
    },
    AdditionKind {
        kind: "ViewStatusAddition",
        edt_tag: "viewStatusAddition",
        edt_type: Some("ViewStatusAddition"),
        ext_xsi: "form:ViewStatusAdditionExtInfo",
        des_source_type: "ViewStatusRepresentation",
    },
    AdditionKind {
        kind: "SearchControlAddition",
        edt_tag: "searchControlAddition",
        edt_type: Some("SearchControlAddition"),
        ext_xsi: "form:SearchControlAdditionExtInfo",
        des_source_type: "SearchControl",
    },
];

/// Найти каталог-строку добавления по каноническому коду.
pub(crate) fn addition_kind(kind: &str) -> Option<&'static AdditionKind> {
    ADDITION_KINDS.iter().find(|k| k.kind == kind)
}

// ============================ каталоги видов ============================

/// Каталог поддержанных типов FormField: канонический вид (= EDT `<type>`) + Designer-тег
/// элемента + extInfo xsi:type + таблица extInfo-полей + Designer-порядок эмиссии.
pub(crate) struct FieldKind {
    /// Канонический код вида (= EDT `<type>`; `InputField`/`SpreadsheetDocumentField`/…).
    pub kind: &'static str,
    /// Designer-тег элемента. Обычно = `kind`; расходится у SpreadsheetDocumentField
    /// (Designer `<SpreadSheetDocumentField>` — регистр `SpreadSheet`, платформенная данность).
    pub des_tag: &'static str,
    /// EDT `<extInfo xsi:type="…">`.
    pub ext_xsi: &'static str,
    /// Таблица extInfo-полей.
    pub ext: &'static [FieldProj],
    /// Designer-порядок эмиссии (пуловый [`DES_FIELD_ORDER`] для большинства; собственный —
    /// у SpreadsheetDocumentField, чей порядок EditMode/геометрии не совпадает с пуловым).
    pub des_order: &'static [DesSlot],
}

/// Поддержанные типы FormField (прочие — громкая §1.0-ошибка).
pub(crate) static FIELD_KINDS: &[FieldKind] = &[
    FieldKind {
        kind: "InputField",
        des_tag: "InputField",
        ext_xsi: "form:InputFieldExtInfo",
        ext: INPUT_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "CheckBoxField",
        des_tag: "CheckBoxField",
        ext_xsi: "form:CheckBoxFieldExtInfo",
        ext: CHECK_BOX_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "LabelField",
        des_tag: "LabelField",
        ext_xsi: "form:LabelFieldExtInfo",
        ext: LABEL_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "RadioButtonField",
        des_tag: "RadioButtonField",
        ext_xsi: "form:RadioButtonsFieldExtInfo",
        ext: RADIO_BUTTON_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "HTMLDocumentField",
        des_tag: "HTMLDocumentField",
        ext_xsi: "form:HtmlFieldExtInfo",
        ext: HTML_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "ProgressBarField",
        des_tag: "ProgressBarField",
        ext_xsi: "form:ProgressBarFieldExtInfo",
        ext: PROGRESS_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "FormattedDocumentField",
        des_tag: "FormattedDocumentField",
        ext_xsi: "form:FormattedDocFieldExtInfo",
        ext: FORMATTED_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "TextDocumentField",
        des_tag: "TextDocumentField",
        ext_xsi: "form:TextDocFieldExtInfo",
        ext: TEXT_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "PictureField",
        des_tag: "PictureField",
        ext_xsi: "form:ImageFieldExtInfo",
        ext: IMAGE_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "SpreadsheetDocumentField",
        des_tag: "SpreadSheetDocumentField",
        ext_xsi: "form:SpreadSheetDocFieldExtInfo",
        ext: SPREADSHEET_FIELD_EXT,
        des_order: DES_SPREADSHEET_ORDER,
    },
    FieldKind {
        kind: "CalendarField",
        des_tag: "CalendarField",
        ext_xsi: "form:CalendarFieldExtInfo",
        ext: CALENDAR_FIELD_EXT,
        des_order: DES_CALENDAR_ORDER,
    },
    FieldKind {
        kind: "TrackBarField",
        des_tag: "TrackBarField",
        ext_xsi: "form:TrackBarFieldExtInfo",
        ext: TRACK_BAR_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "PeriodField",
        des_tag: "PeriodField",
        ext_xsi: "form:PeriodFieldExtInfo",
        ext: PERIOD_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "GraphicalSchemaField",
        des_tag: "GraphicalSchemaField",
        ext_xsi: "form:FlowchartFieldExtInfo",
        ext: FLOWCHART_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    FieldKind {
        kind: "ChartField",
        des_tag: "ChartField",
        ext_xsi: "form:ChartFieldExtInfo",
        ext: CHART_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    // GanttChartField — геометрия ChartField + СТРУКТУРНАЯ авто-таблица (`has_auto_table`);
    // Designer-порядок = DES_FIELD_ORDER (несёт слот `AutoTable` перед `Events`, no-op у прочих).
    FieldKind {
        kind: "GanttChartField",
        des_tag: "GanttChartField",
        ext_xsi: "form:GanttChartFieldExtInfo",
        ext: GANTT_CHART_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
    // PDFDocumentField — геометрия + scale/currentPageNumber + СТРУКТУРНОЕ добавление
    // viewStatusAddition (`field_kind_has_ext_additions`); Designer несёт слот `TableAdditions`.
    FieldKind {
        kind: "PDFDocumentField",
        des_tag: "PDFDocumentField",
        ext_xsi: "form:PDFDocumentFieldExtInfo",
        ext: PDF_DOCUMENT_FIELD_EXT,
        des_order: DES_FIELD_ORDER,
    },
];

/// Виды FormField, чей extInfo несёт АВТО-ТАБЛИЦУ ([`morph1c_core::ir::FormItem::auto_table`]):
/// EDT `<extInfo><autoTable>` ⟷ Designer `<Table>`-ребёнок. Только `GanttChartField` (17 форм
/// ERP). Флаг гонит read/write авто-таблицы + расширяет allowed-списки узлов.
pub(crate) fn field_kind_has_auto_table(kind: &str) -> bool {
    kind == "GanttChartField"
}

/// Виды FormField, чей extInfo несёт ДОБАВЛЕНИЯ ([`morph1c_core::ir::FormItem::additions`]):
/// EDT `<extInfo><viewStatusAddition>` ⟷ Designer `<ViewStatusAddition>`-ребёнок. Только
/// `PDFDocumentField` (4 формы ERP, каждая — один `viewStatusAddition`).
pub(crate) fn field_kind_has_ext_additions(kind: &str) -> bool {
    kind == "PDFDocumentField"
}

/// Найти каталог-строку типа FormField по каноническому коду (= EDT `<type>`).
pub(crate) fn field_kind(kind: &str) -> Option<&'static FieldKind> {
    FIELD_KINDS.iter().find(|k| k.kind == kind)
}

/// Найти каталог-строку типа FormField по Designer-тегу элемента.
pub(crate) fn field_kind_by_des_tag(des_tag: &str) -> Option<&'static FieldKind> {
    FIELD_KINDS.iter().find(|k| k.des_tag == des_tag)
}

/// Виды FormField с WITNESSED композит-шрифтом контрола (`item.font`; Designer `<Font …/>`
/// инлайн ⟺ EDT `extInfo.font`). ERP-счётчики: Input ×613, Radio ×32, Picture ×8, Text ×2,
/// Formatted ×1 (+ SSL LabelField). Прочие виды его не несут — громкий §1.0 на чтении.
/// cf-ячейки: Label ext[10], Input ext[40], Radio ext[3+1=4], Text ext[9], Formatted ext[9]
/// (абляция s10); Picture — cf-НЕ витнесснут (типизированный отказ в build_picture_ext).
pub(crate) fn field_kind_carries_font(kind: &str) -> bool {
    matches!(
        kind,
        "LabelField"
            | "InputField"
            | "RadioButtonField"
            | "TextDocumentField"
            | "FormattedDocumentField"
            | "PictureField"
    )
}

/// Каталог видов семейства FormGroup: канонический вид (= Designer-тег элемента) +
/// EDT `<type>`-дискриминатор (`None` = ButtonGroup, у которого `<type>` ОТСУТСТВУЕТ) +
/// extInfo xsi:type + таблица extInfo-полей.
pub(crate) struct GroupKind {
    /// Канонический код вида.
    pub kind: &'static str,
    /// EDT `<type>`-дискриминатор (`None` — ButtonGroup).
    pub edt_type: Option<&'static str>,
    /// EDT `<extInfo xsi:type="…">`.
    pub ext_xsi: &'static str,
    /// Таблица extInfo-полей.
    pub ext: &'static [FieldProj],
}

/// Поддержанные виды FormGroup (прочие `<type>` — громкая §1.0-ошибка).
pub(crate) static GROUP_KINDS: &[GroupKind] = &[
    GroupKind {
        kind: "UsualGroup",
        edt_type: Some("UsualGroup"),
        ext_xsi: "form:UsualGroupExtInfo",
        ext: USUAL_GROUP_EXT,
    },
    GroupKind {
        kind: "Pages",
        edt_type: Some("Pages"),
        ext_xsi: "form:PagesGroupExtInfo",
        ext: PAGES_EXT,
    },
    GroupKind {
        kind: "Page",
        edt_type: Some("Page"),
        ext_xsi: "form:PageGroupExtInfo",
        ext: PAGE_EXT,
    },
    GroupKind {
        kind: "ButtonGroup",
        edt_type: None,
        ext_xsi: "form:ButtonGroupExtInfo",
        ext: BUTTON_GROUP_EXT,
    },
    GroupKind {
        kind: "CommandBar",
        edt_type: Some("CommandBar"),
        ext_xsi: "form:CommandBarExtInfo",
        ext: COMMAND_BAR_EXT,
    },
    GroupKind {
        kind: "Popup",
        edt_type: Some("Popup"),
        ext_xsi: "form:PopupGroupExtInfo",
        ext: POPUP_EXT,
    },
    GroupKind {
        kind: "ColumnGroup",
        edt_type: Some("ColumnGroup"),
        ext_xsi: "form:ColumnGroupExtInfo",
        ext: COLUMN_GROUP_EXT,
    },
];

/// Найти каталог-строку вида FormGroup по каноническому коду.
pub(crate) fn group_kind(kind: &str) -> Option<&'static GroupKind> {
    GROUP_KINDS.iter().find(|k| k.kind == kind)
}

/// Каталог видов семейства Decoration (`LabelDecoration` — EDT `<type>Label`;
/// `PictureDecoration` — EDT БЕЗ `<type>`).
pub(crate) struct DecorationKind {
    /// Канонический код вида (= Designer-тег элемента).
    pub kind: &'static str,
    /// EDT `<extInfo xsi:type="…">`.
    pub ext_xsi: &'static str,
    /// Таблица extInfo-полей.
    pub ext: &'static [FieldProj],
    /// Designer-порядок эмиссии.
    pub des_order: &'static [DesSlot],
}

/// Поддержанные виды Decoration.
pub(crate) static DECORATION_KINDS: &[DecorationKind] = &[
    DecorationKind {
        kind: "LabelDecoration",
        ext_xsi: "form:LabelDecorationExtInfo",
        ext: LABEL_DECORATION_EXT,
        des_order: DES_LABEL_DECORATION_ORDER,
    },
    DecorationKind {
        kind: "PictureDecoration",
        ext_xsi: "form:PictureDecorationExtInfo",
        ext: PICTURE_DECORATION_EXT,
        des_order: DES_PICTURE_DECORATION_ORDER,
    },
];

/// Найти каталог-строку вида Decoration по каноническому коду.
pub(crate) fn decoration_kind(kind: &str) -> Option<&'static DecorationKind> {
    DECORATION_KINDS.iter().find(|k| k.kind == kind)
}

