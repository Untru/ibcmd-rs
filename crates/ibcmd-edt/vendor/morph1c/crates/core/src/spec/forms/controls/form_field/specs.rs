use super::*;

// ============================ спеки ============================

fn sym(id: FieldId, name: &'static str, kind: ValueKind) -> FieldSpec {
    FieldSpec::required(id, name, kind)
}
fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(
        id,
        name,
        ValueKind::Localized,
        PropertyValue::Localized(Vec::new()),
    )
    .normalized(Normalize::LocalizedSortByLang)
}
fn en(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Enum)
}
fn bl(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Bool)
}
fn nt(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Int)
}
/// Nullable-скаляр xsi-вида ([`ValueKind::Value`]) — общий value_codec (InputField
/// `minValue`/`maxValue`: Number-decimal/String/Undefined, не узкий Int).
fn vv(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Value)
}
fn rf(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Ref)
}

/// Общие свойства FormField в КАНОНИЧЕСКОМ порядке (= порядок EDT-тела, topo 569/569).
fn build_common_fields() -> Vec<FieldSpec> {
    vec![
        en(F_DISPLAY_IMPORTANCE, "displayImportance"),
        loc(F_TITLE, "title"),
        rf(F_TITLE_TEXT_COLOR, "titleTextColor"),
        bl(F_VISIBLE, "visible"),
        bl(F_ENABLED, "enabled"),
        bl(F_USER_VISIBLE, "userVisible"),
        rf(F_DATA_PATH, "dataPath"),
        bl(F_DEFAULT_ITEM, "defaultItem"),
        bl(F_SKIP_ON_INPUT, "skipOnInput"),
        en(F_TITLE_LOCATION, "titleLocation"),
        nt(F_TITLE_HEIGHT, "titleHeight"),
        sym(F_SHORTCUT, "shortcut", ValueKind::Str),
        loc(F_TOOL_TIP, "toolTip"),
        en(F_TOOL_TIP_REPRESENTATION, "toolTipRepresentation"),
        bl(F_READ_ONLY, "readOnly"),
        en(F_HORIZONTAL_ALIGN, "horizontalAlign"),
        en(F_GROUP_HORIZONTAL_ALIGN, "groupHorizontalAlign"),
        en(F_VERTICAL_ALIGN, "verticalAlign"),
        en(F_GROUP_VERTICAL_ALIGN, "groupVerticalAlign"),
        en(
            F_WARNING_ON_EDIT_REPRESENTATION,
            "warningOnEditRepresentation",
        ),
        loc(F_WARNING_ON_EDIT, "warningOnEdit"),
        bl(F_MARK_REQUIRED_COMPLETE, "markRequiredComplete"),
        en(F_EDIT_MODE, "editMode"),
        en(F_FIXING_IN_TABLE, "fixingInTable"),
        bl(F_CELL_HYPERLINK, "cellHyperlink"),
        bl(F_AUTO_CELL_HEIGHT, "autoCellHeight"),
        bl(F_SHOW_IN_HEADER, "showInHeader"),
        rf(F_HEADER_PICTURE, "headerPicture"),
        en(F_HEADER_HORIZONTAL_ALIGN, "headerHorizontalAlign"),
        bl(F_SHOW_IN_FOOTER, "showInFooter"),
        loc(F_FOOTER_TEXT, "footerText"),
        // footerHorizontalAlign ДО autoWidthInTable/cellHyperlink* (метамодель; SSL
        // footerHorizontalAlign<autoWidthInTable×2 — вскрыто ЖурналРегистрации).
        en(F_FOOTER_HORIZONTAL_ALIGN, "footerHorizontalAlign"),
        en(F_AUTO_WIDTH_IN_TABLE, "autoWidthInTable"),
        en(
            F_CELL_HYPERLINK_REPRESENTATION,
            "cellHyperlinkRepresentation",
        ),
        en(
            F_CELL_HYPERLINK_DISPLAY_VARIANT,
            "cellHyperlinkDisplayVariant",
        ),
        bl(F_SHOW_TITLE_IN_CARD, "showTitleInCard"),
    ]
}

/// extInfo InputField в каноническом порядке = ПОРЯДКУ ПОЛЕЙ МЕТАМОДЕЛИ `InputFieldExtInfo`
/// (tools/coverage/form_model.jsonl; индексы в комментариях). EDT-сериализатор эмитит extInfo
/// СТРОГО в порядке метамодели — сверено по 2917/2917 InputFieldExtInfo корпусов SSL+coverage
/// EDT (0 нарушений); прежний topo-порядок CommonForms (288 полей) содержал tie-break-артефакты
/// на не-ко-встречавшихся парах (maxWidth↔height, multiLine/passwordMode↔choiceListButton,
/// editTextUpdate↔choiceList и др.), опровергнутые SSL-витнессами объектных форм.
fn build_input_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),                          // 1
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),          // 2
        nt(F_EXT_MAX_WIDTH, "maxWidth"),                   // 3
        nt(F_EXT_HEIGHT, "height"),                        // 5
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),        // 6
        nt(F_EXT_MAX_HEIGHT, "maxHeight"),                 // 7
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"), // 8
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),     // 9
        bl(F_EXT_WRAP, "wrap"),                            // 10
        bl(F_EXT_PASSWORD_MODE, "passwordMode"),           // 11
        bl(F_EXT_MULTI_LINE, "multiLine"),                 // 12
        bl(F_EXT_EXTENDED_EDIT, "extendedEdit"),           // 13
        bl(
            F_EXT_ALLOW_INPUT_EMPTY_MULTIPLE_VALUES,
            "allowInputEmptyMultipleValues",
        ), // 14
        bl(
            F_EXT_ALLOW_MULTIPLE_VALUES_DUPLICATES,
            "allowMultipleValuesDuplicates",
        ), // 15
        bl(
            F_EXT_EXTENDED_EDIT_MULTIPLE_VALUES,
            "extendedEditMultipleValues",
        ), // 16
        bl(F_EXT_CHOICE_LIST_BUTTON, "choiceListButton"),  // 19
        bl(F_EXT_DROP_LIST_BUTTON, "dropListButton"),      // 20
        bl(F_EXT_CHOICE_BUTTON, "choiceButton"),           // 21
        en(
            F_EXT_CHOICE_BUTTON_REPRESENTATION,
            "choiceButtonRepresentation",
        ), // 22
        rf(F_EXT_PICTURE, "picture"),                      // 25
        rf(F_EXT_CHOICE_BUTTON_PICTURE, "choiceButtonPicture"), // 26
        bl(F_EXT_CLEAR_BUTTON, "clearButton"),             // 27
        bl(F_EXT_SPIN_BUTTON, "spinButton"),               // 28
        bl(F_EXT_OPEN_BUTTON, "openButton"),               // 29
        bl(F_EXT_CREATE_BUTTON, "createButton"),           // 30
        sym(F_EXT_MASK, "mask", ValueKind::Str),           // 31
        bl(F_EXT_AUTO_CHOICE_INCOMPLETE, "autoChoiceIncomplete"), // 32
        bl(F_EXT_QUICK_CHOICE, "quickChoice"),             // 33
        en(F_EXT_CHOICE_FOLDERS_AND_ITEMS, "choiceFoldersAndItems"), // 34
        loc(F_EXT_FORMAT, "format"),                       // 35
        loc(F_EXT_EDIT_FORMAT, "editFormat"),              // 36
        bl(F_EXT_LIST_CHOICE_MODE, "listChoiceMode"),      // 37
        nt(F_EXT_CHOICE_LIST_HEIGHT, "choiceListHeight"),  // 38
        bl(F_EXT_AUTO_MARK_INCOMPLETE, "autoMarkIncomplete"), // 39
        bl(F_EXT_CHOOSE_TYPE, "chooseType"),               // 40
        en(F_EXT_INCOMPLETE_CHOICE_MODE, "incompleteChoiceMode"), // 41
        bl(F_EXT_TYPE_DOMAIN_ENABLED, "typeDomainEnabled"), // 42
        bl(F_EXT_TEXT_EDIT, "textEdit"),                   // 43
        en(F_EXT_EDIT_TEXT_UPDATE, "editTextUpdate"),      // 44
        vv(F_EXT_MIN_VALUE, "minValue"),                   // 45 (Value-скаляр, не Int)
        vv(F_EXT_MAX_VALUE, "maxValue"),                   // 46 (Value-скаляр, не Int)
        rf(F_EXT_CHOICE_FORM, "choiceForm"),               // 47
        FieldSpec::required(
            F_EXT_CHOICE_PARAMETER_LINKS,
            "choiceParameterLinks",
            ValueKind::List,
        ), // 48
        FieldSpec::required(F_EXT_CHOICE_PARAMETERS, "choiceParameters", ValueKind::List), // 49
        sym(F_EXT_AVAILABLE_TYPES, "availableTypes", ValueKind::Type), // 50
        FieldSpec::required(F_EXT_CHOICE_LIST, "choiceList", ValueKind::List), // 51
        rf(F_EXT_TEXT_COLOR, "textColor"),                 // 52
        rf(F_EXT_BACK_COLOR, "backColor"),                 // 53
        rf(F_EXT_BORDER_COLOR, "borderColor"),             // 54
        en(F_EXT_TEXT_SIZE, "textSize"),                   // 55
        FieldSpec::required(F_EXT_TYPE_LINK, "typeLink", ValueKind::List), // 63
        en(F_EXT_HEIGHT_CONTROL_VARIANT, "heightControlVariant"), // 64
        en(F_EXT_AUTO_SHOW_OPEN_BUTTON_MODE, "autoShowOpenButtonMode"), // 66
        en(F_EXT_AUTO_SHOW_CLEAR_BUTTON_MODE, "autoShowClearButtonMode"),
        en(
            F_EXT_AUTO_CORRECTION_ON_TEXT_INPUT,
            "autoCorrectionOnTextInput",
        ), // 67
        en(
            F_EXT_SPELL_CHECKING_ON_TEXT_INPUT,
            "spellCheckingOnTextInput",
        ),
        loc(F_EXT_INPUT_HINT, "inputHint"), // 73
        rf(F_EXT_MULTIPLE_VALUE_DATA_PATH, "multipleValueDataPath"), // 76
        nt(F_EXT_DROP_LIST_WIDTH, "dropListWidth"), // 83
        en(F_EXT_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput"), // 84
    ]
}

/// extInfo CheckBoxField в каноническом порядке (= EDT extInfo).
fn build_check_box_ext_fields() -> Vec<FieldSpec> {
    vec![
        en(F_EXT_CHECK_BOX_TYPE, "checkBoxType"),
        bl(F_EXT_THREE_STATE, "threeState"),
        loc(F_EXT_EDIT_FORMAT, "editFormat"),
    ]
}

/// extInfo LabelField в каноническом порядке = ПОРЯДКУ МЕТАМОДЕЛИ `LabelFieldExtInfo`
/// (геометрия, format, hyperlink, passwordMode, border, textColor, backColor, useCopy).
/// Прежний хвост (textColor/backColor ДО format/hyperlink) был tie-break-артефактом topo
/// (пары не ко-встречались); SSL-витнессы подтверждают метамодель: hyperlink<passwordMode<
/// textColor (ХранилищеВариантовОтчетов), verticalStretch<border<backColor (ПоискИУдалениеДублей),
/// hyperlink<textColor×1, border<backColor×1, backColor<useCopy×2, hyperlink<font<useCopy;
/// контрпримеров 0.
fn build_label_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_MAX_WIDTH, "maxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        nt(F_EXT_MAX_HEIGHT, "maxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        loc(F_EXT_FORMAT, "format"),
        bl(F_EXT_HYPERLINK, "hyperlink"),
        bl(F_EXT_PASSWORD_MODE, "passwordMode"),
        en(F_EXT_LABEL_BORDER, "border"),
        rf(F_EXT_TEXT_COLOR, "textColor"),
        rf(F_EXT_BACK_COLOR, "backColor"),
        bl(F_EXT_USE_COPY, "useCopy"),
    ]
}

/// extInfo HTMLDocumentField (`form:HtmlFieldExtInfo`): геометрия (EDT всегда / Designer
/// опускает) + `output`. Порядок = EDT-эмиссия.
fn build_html_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        en(F_EXT_OUTPUT, "output"),
        rf(F_EXT_BORDER_COLOR, "borderColor"),
    ]
}

/// extInfo ChartField (`form:ChartFieldExtInfo`): только геометрия (width/height — KEEP 50/10,
/// EDT всегда / Designer опускает; autoMax*/stretch — OppositeBool). Witness — DataProcessor
/// ОценкаПроизводительности.ПодборЦелевогоВремениКлючевойОперации.
fn build_chart_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
    ]
}

/// extInfo GanttChartField (`form:GanttChartFieldExtInfo`): та же геометрия, что у ChartField
/// (width/height — KEEP 50/10, EDT всегда / Designer опускает; autoMax*/stretch — OppositeBool).
/// СВЕРХ полей контрол несёт АВТО-ТАБЛИЦУ ([`crate::ir::FormItem::auto_table`], EDT
/// `<autoTable>` ⟷ Designer `<Table>`) и СОБСТВЕННОЕ событие `DetailProcessing` (EDT
/// `<extInfo><handlers>`, опционально) — они живут не в property-bag'е, а в структурных полях
/// узла. Witness — DataProcessor.ДиспетчированиеГрафикаПроизводства.Планирование (17 форм ERP).
fn build_gantt_chart_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
    ]
}

/// extInfo PDFDocumentField (`form:PDFDocumentFieldExtInfo`): геометрия (width/height — KEEP
/// 50/10; autoMax*/stretch — OppositeBool) + `scale` (KEEP Int 100) + `currentPageNumber`
/// (KEEP Int 1). СВЕРХ полей контрол несёт единственное ДОБАВЛЕНИЕ `viewStatusAddition`
/// ([`crate::ir::FormItem::additions`], EDT `<extInfo><viewStatusAddition>` ⟷ Designer
/// `<ViewStatusAddition>`). Порядок = EDT-эмиссия. Witness — DataProcessor.СервисДоставки.ПросмотрPDF
/// (4 формы ERP; все значения констант — до-майнятся при cf-приёмке).
fn build_pdf_document_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        nt(F_EXT_SCALE, "scale"),
        nt(F_EXT_CURRENT_PAGE_NUMBER, "currentPageNumber"),
    ]
}

/// extInfo GraphicalSchemaField (`form:FlowchartFieldExtInfo`): геометрия (width/height —
/// Symmetric; autoMax*/stretch — OppositeBool) + `edit` (OppositeBool: EDT опускает / Designer
/// эмитит `false`). Порядок = EDT-эмиссия. Witness — DataProcessor КартаМаршрутаБизнесПроцесса.
fn build_flowchart_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        bl(F_EXT_EDIT, "edit"),
    ]
}

/// extInfo ProgressBarField (`form:ProgressBarFieldExtInfo`): геометрия + `verticalStretch`
/// (OppositeBool; EDT эмитит true / Designer опускает) + `maxValue` (plain Int) + `showPercent`.
fn build_progress_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_MAX_WIDTH, "maxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        nt(F_EXT_MAX_VALUE, "maxValue"),
        en(F_EXT_PROGRESS_REPRESENTATION, "representation"),
        bl(F_EXT_SHOW_PERCENT, "showPercent"),
        en(F_EXT_ORIENTATION, "orientation"),
    ]
}

/// extInfo TrackBarField (`form:TrackBarFieldExtInfo`): геометрия (EDT всегда / Designer
/// опускает тип-дефолты) + maxValue/step/largeStep/markingStep/markingAppearance + orientation.
/// Порядок = EDT-эмиссия (форма `Форма_ПоляИндикаторы`).
fn build_trackbar_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        nt(F_EXT_MAX_VALUE, "maxValue"),
        nt(F_EXT_STEP, "step"),
        nt(F_EXT_LARGE_STEP, "largeStep"),
        nt(F_EXT_MARKING_STEP, "markingStep"),
        en(F_EXT_ORIENTATION, "orientation"),
        en(F_EXT_MARKING_APPEARANCE, "markingAppearance"),
    ]
}

/// extInfo PeriodField (`form:PeriodFieldExtInfo`): autoMax*/stretch + `border` (BorderDef,
/// EDT всегда `Single` / Designer опускает). БЕЗ геометрии width/height (сверено корпусом).
fn build_period_ext_fields() -> Vec<FieldSpec> {
    vec![
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        en(F_EXT_BORDER, "border"),
    ]
}

/// extInfo FormattedDocumentField (`form:FormattedDocFieldExtInfo`): геометрия (EDT всегда /
/// Designer опускает) + `output` (Enum, симметрично; дефолт `Use` опускается — сверено корпусом).
fn build_formatted_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        en(F_EXT_OUTPUT, "output"),
    ]
}

/// extInfo TextDocumentField (`form:TextDocFieldExtInfo`): та же геометрия, что у
/// FormattedDocumentField, ПЛЮС `output` (Enum, симметрично; дефолт `Use` опускается) —
/// сверено корпусом покрытия. Порядок = EDT-эмиссия (output — после verticalStretch).
fn build_text_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        en(F_EXT_OUTPUT, "output"),
    ]
}

/// extInfo PictureField (`form:ImageFieldExtInfo`): геометрия + pictureSize/hyperlink/
/// valuesPicture/border/fileDragMode. Порядок = EDT-эмиссия (topo по корпусу).
fn build_image_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_MAX_WIDTH, "maxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        en(F_EXT_PICTURE_SIZE, "pictureSize"),
        bl(F_EXT_PIC_HYPERLINK, "hyperlink"),
        loc(F_EXT_NONSELECTED_PICTURE_TEXT, "nonselectedPictureText"),
        rf(F_EXT_PICTURE_COLOR, "pictureColor"),
        rf(F_EXT_VALUES_PICTURE, "valuesPicture"),
        rf(F_EXT_PIC_TEXT_COLOR, "textColor"),
        en(F_EXT_BORDER, "border"),
        en(F_EXT_FILE_DRAG_MODE, "fileDragMode"),
    ]
}

/// extInfo SpreadsheetDocumentField (`form:SpreadSheetDocFieldExtInfo`): геометрия +
/// сетка/заголовки + указатель/скроллбары/выделение + группы/перетаскивание. Порядок =
/// EDT-эмиссия (topo 12/12).
fn build_spreadsheet_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        nt(F_EXT_MAX_HEIGHT, "maxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        bl(F_EXT_SHOW_GRID, "showGrid"),
        bl(F_EXT_SHOW_HEADERS, "showHeaders"),
        bl(F_EXT_SHOW_CELL_NAMES, "showCellNames"),
        bl(F_EXT_SHOW_ROW_AND_COLUMN_NAMES, "showRowAndColumnNames"),
        en(F_EXT_POINTER_TYPE, "pointerType"),
        en(F_EXT_VERTICAL_SCROLL_BAR, "verticalScrollBar"),
        en(F_EXT_HORIZONTAL_SCROLL_BAR, "horizontalScrollBar"),
        bl(F_EXT_PROTECTION, "protection"),
        en(F_EXT_SELECTION_SHOW_MODE, "selectionShowMode"),
        en(
            F_EXT_DRAWING_SELECTION_SHOW_MODE,
            "drawingSelectionShowMode",
        ),
        en(F_EXT_OUTPUT, "output"),
        bl(F_EXT_EDIT, "edit"),
        bl(F_EXT_SHOW_GROUPS, "showGroups"),
        bl(F_EXT_ENABLE_START_DRAG, "enableStartDrag"),
        bl(F_EXT_ENABLE_DRAG, "enableDrag"),
        rf(F_EXT_BORDER_COLOR, "borderColor"),
    ]
}

/// extInfo CalendarField (`form:CalendarFieldExtInfo`, LANE-F-7): геометрия +
/// calendarNavigation/showCurrentDate/border/widthInMonths/heightInMonths. Порядок =
/// EDT-эмиссия (форма `ВыборДаты`).
fn build_calendar_ext_fields() -> Vec<FieldSpec> {
    vec![
        nt(F_EXT_WIDTH, "width"),
        bl(F_EXT_AUTO_MAX_WIDTH, "autoMaxWidth"),
        nt(F_EXT_HEIGHT, "height"),
        bl(F_EXT_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        bl(F_EXT_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_EXT_VERTICAL_STRETCH, "verticalStretch"),
        en(F_EXT_CALENDAR_SELECTION_MODE, "selectionMode"),
        // Порядок сверен корпусом покрытия: showCurrentDate ПЕРЕД calendarNavigation.
        bl(F_EXT_SHOW_CURRENT_DATE, "showCurrentDate"),
        bl(F_EXT_CALENDAR_NAVIGATION, "calendarNavigation"),
        // enableStartDrag/enableDrag (Symmetric; witness Форма_ПоляВвода) — после calendarNavigation.
        bl(F_EXT_ENABLE_START_DRAG, "enableStartDrag"),
        bl(F_EXT_ENABLE_DRAG, "enableDrag"),
        en(F_EXT_BORDER, "border"),
        bl(F_EXT_SHOW_MONTHS_PANEL, "showMonthsPanel"),
        nt(F_EXT_WIDTH_IN_MONTHS, "widthInMonths"),
        nt(F_EXT_HEIGHT_IN_MONTHS, "heightInMonths"),
    ]
}

fn leak_spec(entity: &'static str, fields: Vec<FieldSpec>) -> EntitySpec {
    EntitySpec {
        entity,
        fields: Box::leak(fields.into_boxed_slice()),
        children: &[],
    }
}

/// Канонический спек ОБЩИХ свойств семейства FormField (кэш на процесс, общий всем типам).
pub fn form_field_common() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("FormField", build_common_fields()))
}

fn input_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("InputFieldExtInfo", build_input_ext_fields()))
}
fn check_box_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("CheckBoxFieldExtInfo", build_check_box_ext_fields()))
}
fn label_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("LabelFieldExtInfo", build_label_ext_fields()))
}

/// Канонический [`ControlSpec`] вида `InputField` (лист; данные-привязанное поле ввода).
pub fn input_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "InputField",
        properties: form_field_common(),
        ext_info: input_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `CheckBoxField` (лист).
pub fn check_box_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "CheckBoxField",
        properties: form_field_common(),
        ext_info: check_box_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `LabelField` (лист).
pub fn label_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "LabelField",
        properties: form_field_common(),
        ext_info: label_ext_spec(),
        container: false,
    })
}

fn html_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("HtmlFieldExtInfo", build_html_ext_fields()))
}
fn progress_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("ProgressBarFieldExtInfo", build_progress_ext_fields()))
}
fn formatted_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("FormattedDocFieldExtInfo", build_formatted_ext_fields()))
}
fn text_ext_spec() -> &'static EntitySpec {
    // TextDocFieldExtInfo = геометрия FormattedDoc + `output` (см. build_text_ext_fields).
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("TextDocFieldExtInfo", build_text_ext_fields()))
}
fn image_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("ImageFieldExtInfo", build_image_ext_fields()))
}
fn spreadsheet_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("SpreadSheetDocFieldExtInfo", build_spreadsheet_ext_fields()))
}
fn calendar_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("CalendarFieldExtInfo", build_calendar_ext_fields()))
}
fn trackbar_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("TrackBarFieldExtInfo", build_trackbar_ext_fields()))
}
fn period_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("PeriodFieldExtInfo", build_period_ext_fields()))
}
fn flowchart_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("FlowchartFieldExtInfo", build_flowchart_ext_fields()))
}
fn chart_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("ChartFieldExtInfo", build_chart_ext_fields()))
}
fn gantt_chart_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("GanttChartFieldExtInfo", build_gantt_chart_ext_fields()))
}
fn pdf_document_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("PDFDocumentFieldExtInfo", build_pdf_document_ext_fields()))
}

/// Канонический [`ControlSpec`] вида `ChartField` (лист): общее тело FormField + геометрия
/// `form:ChartFieldExtInfo`.
pub fn chart_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "ChartField",
        properties: form_field_common(),
        ext_info: chart_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `GanttChartField` (лист по `container`-флагу, НО несёт
/// авто-таблицу в [`crate::ir::FormItem::auto_table`]): общее тело FormField + геометрия
/// `form:GanttChartFieldExtInfo`. Родня ChartField (та же геометрия extInfo); авто-таблица и
/// событие `DetailProcessing` — структурные поля узла, не property-bag.
pub fn gantt_chart_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "GanttChartField",
        properties: form_field_common(),
        ext_info: gantt_chart_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `PDFDocumentField` (лист; поле просмотра PDF): общее тело
/// FormField + `form:PDFDocumentFieldExtInfo` (геометрия + scale + currentPageNumber). Несёт
/// единственное добавление `viewStatusAddition` в [`crate::ir::FormItem::additions`].
pub fn pdf_document_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "PDFDocumentField",
        properties: form_field_common(),
        ext_info: pdf_document_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `GraphicalSchemaField` (лист): общее тело FormField +
/// геометрия `form:FlowchartFieldExtInfo`.
pub fn graphical_schema_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "GraphicalSchemaField",
        properties: form_field_common(),
        ext_info: flowchart_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `HTMLDocumentField` (лист).
pub fn html_document_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "HTMLDocumentField",
        properties: form_field_common(),
        ext_info: html_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `ProgressBarField` (лист).
pub fn progress_bar_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "ProgressBarField",
        properties: form_field_common(),
        ext_info: progress_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `FormattedDocumentField` (лист).
pub fn formatted_document_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "FormattedDocumentField",
        properties: form_field_common(),
        ext_info: formatted_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `TextDocumentField` (лист; поле текстового документа).
/// extInfo (`form:TextDocFieldExtInfo`) — та же геометрия, что у FormattedDocumentField.
pub fn text_document_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "TextDocumentField",
        properties: form_field_common(),
        ext_info: text_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `PictureField` (лист).
pub fn picture_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "PictureField",
        properties: form_field_common(),
        ext_info: image_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `SpreadsheetDocumentField` (лист). Designer-тег
/// расходится с каноном по регистру (`SpreadSheetDocumentField`) — проекция несёт `des_tag`.
pub fn spreadsheet_document_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "SpreadsheetDocumentField",
        properties: form_field_common(),
        ext_info: spreadsheet_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `CalendarField` (лист; поле-календарь, LANE-F-7).
pub fn calendar_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "CalendarField",
        properties: form_field_common(),
        ext_info: calendar_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `TrackBarField` (лист; поле-регулятор).
pub fn track_bar_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "TrackBarField",
        properties: form_field_common(),
        ext_info: trackbar_ext_spec(),
        container: false,
    })
}

/// Канонический [`ControlSpec`] вида `PeriodField` (лист; поле периода).
pub fn period_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "PeriodField",
        properties: form_field_common(),
        ext_info: period_ext_spec(),
        container: false,
    })
}

/// Неподдержанные (пока) типы FormField корпуса — для громкой §1.0-диагностики по частоте.
/// Все витнессированные в CommonForms типы FormField теперь смоделированы (пусто).
pub const UNSUPPORTED_FIELD_TYPES: &[&str] = &[];
