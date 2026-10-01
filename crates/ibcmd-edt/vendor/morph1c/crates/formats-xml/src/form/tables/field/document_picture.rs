//! extInfo document/picture-подтипов FormField (LANE-F-5): HTML/Progress/Formatted/Text/
//! Image/Spreadsheet/Calendar/TrackBar/Chart/Gantt/PDF/Flowchart/Period.

use crate::form::fields::{fp, Codec, DesOmit, FieldProj, Policy, Region};
use crate::form::tables::{
    geo_auto_max_height, geo_auto_max_width, geo_h_stretch, geo_height, geo_max_height,
    geo_max_width, geo_v_stretch, geo_width, keep, F_EXT_PIC_ENABLE_DRAG, F_EXT_VIEW_SCALING_MODE,
    F_EXT_ZOOMABLE, F_PIC_BORDER_COLOR,
};
use morph1c_core::spec::forms::controls::form_field as ff;

// ============ document/picture FormField-подтипы (LANE-F-5) ============
// Геометрия HTML/Progress/Formatted: EDT эмитит ВСЕГДА тип-дефолт, Designer ОПУСКАЕТ (все
// дефолтные) — KEEP (Int) / OppositeBool. У PictureField геометрия — Symmetric (оба эмитят).

/// extInfo HTMLDocumentField (`form:HtmlFieldExtInfo`). Геометрия — тип-дефолты (EDT всегда /
/// Designer опускает); `output` — симметричный enum (оба опускают дефолт `Use`). extInfo несёт
/// событие `OnClick` (общий handler-механизм). Порядок = EDT.
pub(crate) static HTML_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("50", None, "50", DesOmit::Eq("50"))),
    geo_auto_max_width(Policy::OppositeBool),
    // maxWidth/maxHeight — Symmetric Int (метамодель HTML: autoMaxWidth→maxWidth,
    // autoMaxHeight→maxHeight; ERP 6⟷6 и 11⟷11). cf: HTML ext[7]/[10] (абляция s10).
    geo_max_width(Policy::Symmetric),
    geo_height(keep("10", None, "10", DesOmit::Eq("10"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_max_height(Policy::Symmetric),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    fp(
        ff::F_EXT_OUTPUT,
        "output",
        "Output",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // borderColor — Color Symmetric, ПОСЛЕ output (witness HTMLDocumentField УправлениеПодключениемDSS
    // borderColor=Style.FormBackColor; output опущен ⇒ borderColor хвостовой).
    fp(
        ff::F_EXT_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
];

/// extInfo ProgressBarField (`form:ProgressBarFieldExtInfo`). Геометрия (без verticalStretch)
/// + `maxValue` (plain Int, тип-дефолт 100) + `showPercent` (Designer эмитит).
pub(crate) static PROGRESS_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("32", None, "32", DesOmit::Eq("32"))),
    geo_auto_max_width(Policy::OppositeBool),
    geo_max_width(Policy::Symmetric),
    geo_height(keep("1", None, "1", DesOmit::Eq("1"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    // verticalStretch — Symmetric (дефолт false; оба эмитят `true`, оба опускают — как TrackBar).
    geo_v_stretch(Policy::Symmetric),
    fp(
        ff::F_EXT_MAX_VALUE,
        "maxValue",
        "MaxValue",
        Region::Ext,
        Codec::Int,
        keep("100", None, "100", DesOmit::Eq("100")),
    ),
    // representation — Symmetric (оба эмитят `Broken`/`BrokenTilt`, оба опускают дефолт `Auto`).
    fp(
        ff::F_EXT_PROGRESS_REPRESENTATION,
        "representation",
        "Representation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SHOW_PERCENT,
        "showPercent",
        "ShowPercent",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // orientation — Symmetric (оба эмитят `Vertical`, оба опускают дефолт `Horizontal`).
    fp(
        ff::F_EXT_ORIENTATION,
        "orientation",
        "Orientation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// extInfo FormattedDocumentField (`form:FormattedDocFieldExtInfo`): только геометрия (EDT
/// всегда тип-дефолт / Designer опускает). Дефолты: `width`=50, `height`=10 (сверено по
/// корпусу: ФормаПоиска несёт EDT width=50/height=10 и Designer ОБА опускает; формы с иными
/// значениями — ВосстановлениеПаролей height=5, ОтправкаСообщения width=70/height=5,
/// РедактированиеТабличногоДокумента width=20/height=2 — эмитят их явно).
pub(crate) static FORMATTED_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("50", None, "50", DesOmit::Eq("50"))),
    geo_auto_max_width(Policy::OppositeBool),
    geo_height(keep("10", None, "10", DesOmit::Eq("10"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    // `output` — симметричный enum (оба опускают дефолт `Use`; witness Форма_ПоляДокументы).
    fp(
        ff::F_EXT_OUTPUT,
        "output",
        "Output",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // borderColor — Color Symmetric (ERP 8⟷8, всё style:FormBackColor). cf: Formatted ext[8]
    // (абляция s10: {4,4,{0},4}→{4,3,{-1},3}).
    fp(
        ff::F_EXT_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
];

/// extInfo TextDocumentField (`form:TextDocFieldExtInfo`): геометрия идентична
/// FormattedDocumentField (дефолты width=50/height=10; сверено РедактированиеТабличногоДокумента:
/// EDT width=50/height=10 Designer ОБА опускает; height=3 эмитит явно). EDT всегда / Designer опускает.
pub(crate) static TEXT_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("50", None, "50", DesOmit::Eq("50"))),
    geo_auto_max_width(Policy::OppositeBool),
    // maxWidth/maxHeight — Symmetric Int (ERP 3⟷3 и 1⟷1). cf: Text ext[11]/[14] (абляция s10).
    geo_max_width(Policy::Symmetric),
    geo_height(keep("10", None, "10", DesOmit::Eq("10"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_max_height(Policy::Symmetric),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    // `output` — симметричный enum (оба опускают дефолт `Use`; witness Форма_ПоляДокументы).
    fp(
        ff::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_OUTPUT,
        "output",
        "Output",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// extInfo PictureField (`form:ImageFieldExtInfo`). Геометрия — Symmetric (оба эмитят
/// фактические значения); autoMax*/stretch — OppositeBool; `valuesPicture` — PictureRef;
/// `border` — KEEP-композит (EDT всегда / Designer опускает `Single`); `fileDragMode` — KEEP
/// (AsFile/AsFileRef). Порядок = EDT (topo по корпусу).
pub(crate) static IMAGE_FIELD_EXT: &[FieldProj] = &[
    geo_width(Policy::Symmetric),
    geo_auto_max_width(Policy::OppositeBool),
    geo_max_width(Policy::Symmetric),
    geo_height(Policy::Symmetric),
    geo_auto_max_height(Policy::OppositeBool),
    // maxHeight — Symmetric Int (ERP 21⟷21; cf-ячейка НЕ витнесснута — типизированный отказ).
    geo_max_height(Policy::Symmetric),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    fp(
        ff::F_EXT_PICTURE_SIZE,
        "pictureSize",
        "PictureSize",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_PIC_HYPERLINK,
        "hyperlink",
        "Hyperlink",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_NONSELECTED_PICTURE_TEXT,
        "nonselectedPictureText",
        "NonselectedPictureText",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    // enableDrag — Symmetric Bool (метамодель ImageField: nonselectedPictureText(16)→
    // enableStartDrag(17)→enableDrag(18); ERP 2⟷2×true; cf-ячейка НЕ витнесснута — отказ).
    fp(
        F_EXT_PIC_ENABLE_DRAG,
        "enableDrag",
        "EnableDrag",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // pictureColor — Color Symmetric, ПЕРЕД valuesPicture (witness DocumentJournal Взаимодействия
    // ImageFieldExtInfo pictureColor=Palette.Red перед valuesPicture).
    fp(
        ff::F_EXT_PICTURE_COLOR,
        "pictureColor",
        "PictureColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // zoomable — Symmetric Bool (метамодель ImageField: pictureColor(12)→zoomable(13);
    // ERP 9⟷21 (у декораций свой известный zoomable); cf-ячейка НЕ витнесснута — отказ).
    fp(
        F_EXT_ZOOMABLE,
        "zoomable",
        "Zoomable",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_VALUES_PICTURE,
        "valuesPicture",
        "ValuesPicture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    // textColor — Color Symmetric, ПОСЛЕ valuesPicture, ДО border (witness ImageFieldExtInfo
    // Взаимодействия textColor=Palette.Red после valuesPicture; собственный id 245).
    fp(
        ff::F_EXT_PIC_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_BORDER,
        "border",
        "Border",
        Region::Ext,
        Codec::Border,
        keep(
            ff::BORDER_STYLE_SINGLE,
            None,
            ff::BORDER_STYLE_SINGLE,
            DesOmit::Eq(ff::BORDER_STYLE_SINGLE),
        ),
    ),
    // borderColor — Color Symmetric, СРАЗУ за `border` в EDT (witness Новости.ФормаНовости
    // `border`→`borderColor`=Style.BorderColor; ERP-волна). СОБСТВЕННЫЙ id [`F_PIC_BORDER_COLOR`]
    // (Designer-порядок иной — см. DES_FIELD_ORDER). cf-ячейка НЕ витнесснута — эмит по
    // witness-ключу, см. build_picture_ext.
    fp(
        F_PIC_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_FILE_DRAG_MODE,
        "fileDragMode",
        "FileDragMode",
        Region::Ext,
        Codec::EnumTok,
        keep(
            ff::FILE_DRAG_MODE_EDT_DEFAULT,
            Some(ff::FILE_DRAG_MODE_EDT_DEFAULT),
            ff::FILE_DRAG_MODE_DESIGNER_DEFAULT,
            DesOmit::Eq(ff::FILE_DRAG_MODE_DESIGNER_DEFAULT),
        ),
    ),
];

/// extInfo SpreadsheetDocumentField (`form:SpreadSheetDocFieldExtInfo`, LANE-F-6). Порядок =
/// EDT-эмиссия (topo 12/12). Кросс-кодировки/дефолты — cross-omission witness по 7 формам:
/// * `width`/`height` — EDT всегда / Designer опускает 50/10 (KEEP Int).
/// * `pointerType`/`drawingSelectionShowMode`/`showGroups` — EDT всегда / Designer НИКОГДА.
/// * `selectionShowMode` — ПРОТИВОПОЛОЖНЫЕ дефолты (EDT опускает `WhenActive`, Designer `Always`).
/// * `verticalScrollBar`/`horizontalScrollBar` — [`Codec::ScrollBar`] (enum↔bool) + KEEP
///   (EDT опускает `ScrollNever`, Designer — `ScrollAuto`).
/// * `autoMax*`/`stretch`/`enable*Drag` — OppositeBool; `maxHeight`/`showGrid`/`showHeaders`/
///   `showCellNames`/`showRowAndColumnNames`/`output`/`edit` — Symmetric.
pub(crate) static SPREADSHEET_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep(
        ff::SS_WIDTH_DEFAULT,
        None,
        ff::SS_WIDTH_DEFAULT,
        DesOmit::Eq(ff::SS_WIDTH_DEFAULT),
    )),
    geo_auto_max_width(Policy::OppositeBool),
    // maxWidth — Symmetric Int (ERP 19⟷19; cf-ячейка НЕ витнесснута для Spreadsheet —
    // типизированный отказ cf-write).
    geo_max_width(Policy::Symmetric),
    geo_height(keep(
        ff::SS_HEIGHT_DEFAULT,
        None,
        ff::SS_HEIGHT_DEFAULT,
        DesOmit::Eq(ff::SS_HEIGHT_DEFAULT),
    )),
    geo_auto_max_height(Policy::OppositeBool),
    geo_max_height(Policy::Symmetric),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    fp(
        ff::F_EXT_SHOW_GRID,
        "showGrid",
        "ShowGrid",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SHOW_HEADERS,
        "showHeaders",
        "ShowHeaders",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SHOW_CELL_NAMES,
        "showCellNames",
        "ShowCellNames",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SHOW_ROW_AND_COLUMN_NAMES,
        "showRowAndColumnNames",
        "ShowRowAndColumnNames",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_POINTER_TYPE,
        "pointerType",
        "PointerType",
        Region::Ext,
        Codec::EnumTok,
        keep(
            ff::POINTER_TYPE_FILL,
            None,
            ff::POINTER_TYPE_FILL,
            DesOmit::Always,
        ),
    ),
    fp(
        ff::F_EXT_VERTICAL_SCROLL_BAR,
        "verticalScrollBar",
        "VerticalScrollBar",
        Region::Ext,
        Codec::ScrollBar,
        keep(
            ff::SCROLL_BAR_EDT_DEFAULT,
            Some(ff::SCROLL_BAR_EDT_DEFAULT),
            ff::SCROLL_BAR_DESIGNER_DEFAULT,
            DesOmit::Eq(ff::SCROLL_BAR_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        ff::F_EXT_HORIZONTAL_SCROLL_BAR,
        "horizontalScrollBar",
        "HorizontalScrollBar",
        Region::Ext,
        Codec::ScrollBar,
        keep(
            ff::SCROLL_BAR_EDT_DEFAULT,
            Some(ff::SCROLL_BAR_EDT_DEFAULT),
            ff::SCROLL_BAR_DESIGNER_DEFAULT,
            DesOmit::Eq(ff::SCROLL_BAR_DESIGNER_DEFAULT),
        ),
    ),
    // protection — Bool Symmetric (оба эмитят true, оба опускают false; witness СнимкиОтчетов ×2:
    // EDT horizontalScrollBar→protection→selectionShowMode; метамодель SpreadSheetDocFieldExtInfo).
    fp(
        ff::F_EXT_PROTECTION,
        "protection",
        "Protection",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SELECTION_SHOW_MODE,
        "selectionShowMode",
        "SelectionShowMode",
        Region::Ext,
        Codec::EnumTok,
        keep(
            ff::SELECTION_SHOW_MODE_EDT_DEFAULT,
            Some(ff::SELECTION_SHOW_MODE_EDT_DEFAULT),
            ff::SELECTION_SHOW_MODE_DESIGNER_DEFAULT,
            DesOmit::Eq(ff::SELECTION_SHOW_MODE_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        ff::F_EXT_DRAWING_SELECTION_SHOW_MODE,
        "drawingSelectionShowMode",
        "DrawingSelectionShowMode",
        Region::Ext,
        // Installed Form.xcore declares precisely Show/DontShow/Auto. Native UH
        // carries Show explicitly: omitting every value destroyed this setting.
        Codec::EnumMap(&[("Show", "Show"), ("DontShow", "DontShow"), ("Auto", "Auto")]),
        keep(
            ff::DRAWING_SELECTION_SHOW_MODE_FILL,
            None,
            ff::DRAWING_SELECTION_SHOW_MODE_FILL,
            DesOmit::Eq(ff::DRAWING_SELECTION_SHOW_MODE_FILL),
        ),
    ),
    fp(
        ff::F_EXT_OUTPUT,
        "output",
        "Output",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_EDIT,
        "edit",
        "Edit",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SHOW_GROUPS,
        "showGroups",
        "ShowGroups",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_ENABLE_START_DRAG,
        "enableStartDrag",
        "EnableStartDrag",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_ENABLE_DRAG,
        "enableDrag",
        "EnableDrag",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // viewScalingMode — Symmetric Enum (ERP 1759⟷1759×Normal; EDT-witness
    // ГосударственныеКонтракты: enableDrag→viewScalingMode последним в extInfo; Designer —
    // HorizontalScrollBar→ViewScalingMode→ContextMenu). cf: ext[19] (абляция s10 0→1).
    fp(
        F_EXT_VIEW_SCALING_MODE,
        "viewScalingMode",
        "ViewScalingMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // borderColor — Color Symmetric, ЗАМЫКАЕТ extInfo (метамодель …enableDrag→borderColor;
    // witness УправлениеПодключениемDSS.ПодтверждениеПользователя Style.FormBackColor ⟷
    // style:FormBackColor, Designer SelectionShowMode→BorderColor→ContextMenu).
    fp(
        ff::F_EXT_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
];

/// extInfo CalendarField (`form:CalendarFieldExtInfo`, LANE-F-7). Порядок = EDT-эмиссия
/// (форма `ВыборДаты`). Кросс-омиссии/дефолты — cross-omission witness:
/// * `width`/`height`/`widthInMonths` — Symmetric (оба эмитят фактическое значение).
/// * `autoMax*`/`stretch`/`calendarNavigation` — OppositeBool (EDT true / Designer default true).
/// * `showCurrentDate` — OppositeBool (EDT default false / Designer эмитит false).
/// * `border` — KEEP-композит `Border` (EDT всегда `Single` / Designer опускает `Single`).
/// * `heightInMonths` — KEEP Int (EDT эмитит ВСЕГДА / Designer опускает дефолт `1`).
pub(crate) static CALENDAR_FIELD_EXT: &[FieldProj] = &[
    // width/height: EDT эмитит ВСЕГДА тип-дефолт (16/9), Designer опускает (сверено корпусом покрытия).
    geo_width(keep("16", None, "16", DesOmit::Eq("16"))),
    geo_auto_max_width(Policy::OppositeBool),
    geo_height(keep("9", None, "9", DesOmit::Eq("9"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    // selectionMode ПЕРЕД showCurrentDate; showCurrentDate ПЕРЕД calendarNavigation (сверено корпусом).
    fp(
        ff::F_EXT_CALENDAR_SELECTION_MODE,
        "selectionMode",
        "SelectionMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SHOW_CURRENT_DATE,
        "showCurrentDate",
        "ShowCurrentDate",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_CALENDAR_NAVIGATION,
        "calendarNavigation",
        "CalendarNavigation",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_ENABLE_START_DRAG,
        "enableStartDrag",
        "EnableStartDrag",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_ENABLE_DRAG,
        "enableDrag",
        "EnableDrag",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_BORDER,
        "border",
        "Border",
        Region::Ext,
        Codec::Border,
        keep(
            ff::BORDER_STYLE_SINGLE,
            None,
            ff::BORDER_STYLE_SINGLE,
            DesOmit::Eq(ff::BORDER_STYLE_SINGLE),
        ),
    ),
    fp(
        ff::F_EXT_SHOW_MONTHS_PANEL,
        "showMonthsPanel",
        "ShowMonthsPanel",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // width/heightInMonths: канон 0 = АВТО. EDT: absent ⟺ 0 (опускает 0, эмитит 1..N —
    // витнессы Календари absent⟺Designer 0/0; Хранилище… 1/1 явные; ПроизводственныеКалендари
    // width=4+height-absent ⟺ Designer 4/0). Designer: absent ⟺ 1 (опускает 1, эмитит 0 и N≥2).
    fp(
        ff::F_EXT_WIDTH_IN_MONTHS,
        "widthInMonths",
        "WidthInMonths",
        Region::Ext,
        Codec::Int,
        keep(
            "0",
            Some("0"),
            ff::HEIGHT_IN_MONTHS_DEFAULT,
            DesOmit::Eq(ff::HEIGHT_IN_MONTHS_DEFAULT),
        ),
    ),
    fp(
        ff::F_EXT_HEIGHT_IN_MONTHS,
        "heightInMonths",
        "HeightInMonths",
        Region::Ext,
        Codec::Int,
        keep(
            "0",
            Some("0"),
            ff::HEIGHT_IN_MONTHS_DEFAULT,
            DesOmit::Eq(ff::HEIGHT_IN_MONTHS_DEFAULT),
        ),
    ),
];

/// extInfo TrackBarField (`form:TrackBarFieldExtInfo`): геометрия + maxValue/step/largeStep/
/// markingStep/markingAppearance + orientation. EDT эмитит тип-дефолты ВСЕГДА; Designer их
/// ОПУСКАЕТ (KEEP), эмитит лишь не-дефолт. Порядок = EDT (форма Форма_ПоляИндикаторы).
pub(crate) static TRACK_BAR_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep(
        ff::TRACK_BAR_WIDTH_DEFAULT,
        None,
        ff::TRACK_BAR_WIDTH_DEFAULT,
        DesOmit::Eq(ff::TRACK_BAR_WIDTH_DEFAULT),
    )),
    geo_auto_max_width(Policy::OppositeBool),
    // maxWidth — Symmetric Int (ERP-witness ГрафикиРаботыПерсонала 1⟷1: оба эмитят
    // не-дефолт; SSL/coverage не несут). cf-ячейка: ext[14] (абляция s10 0→20).
    geo_max_width(Policy::Symmetric),
    geo_height(keep(
        ff::TRACK_BAR_HEIGHT_DEFAULT,
        None,
        ff::TRACK_BAR_HEIGHT_DEFAULT,
        DesOmit::Eq(ff::TRACK_BAR_HEIGHT_DEFAULT),
    )),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    // verticalStretch — Symmetric (дефолт false; оба эмитят `true`, оба опускают false —
    // witness Регулятор_ВертикальноеРастяжение; отличается от ProgressBar/HTML, где OppositeBool).
    geo_v_stretch(Policy::Symmetric),
    // minValue — Symmetric Int (метамодель TrackBarFieldExtInfo: verticalStretch→minValue→
    // maxValue; ERP 4⟷4 — оба диалекта эмитят лишь не-дефолт 0; Designer-witness
    // МоделиПооперационногоПланирования: Width→Height→MinValue→MaxValue). РЕИСПОЛЬЗУЕТ
    // ff::F_EXT_MIN_VALUE (131 — InputField NumberValue-канон; здесь Int-кодек, как maxValue).
    // cf-ячейка: ext[5] (абляция s10 0→20).
    fp(
        ff::F_EXT_MIN_VALUE,
        "minValue",
        "MinValue",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_MAX_VALUE,
        "maxValue",
        "MaxValue",
        Region::Ext,
        Codec::Int,
        keep(
            ff::TRACK_BAR_MAX_VALUE_DEFAULT,
            None,
            ff::TRACK_BAR_MAX_VALUE_DEFAULT,
            DesOmit::Eq(ff::TRACK_BAR_MAX_VALUE_DEFAULT),
        ),
    ),
    fp(
        ff::F_EXT_STEP,
        "step",
        "Step",
        Region::Ext,
        Codec::Int,
        keep(
            ff::TRACK_BAR_STEP_DEFAULT,
            None,
            ff::TRACK_BAR_STEP_DEFAULT,
            DesOmit::Eq(ff::TRACK_BAR_STEP_DEFAULT),
        ),
    ),
    fp(
        ff::F_EXT_LARGE_STEP,
        "largeStep",
        "LargeStep",
        Region::Ext,
        Codec::Int,
        keep(
            ff::TRACK_BAR_LARGE_STEP_DEFAULT,
            None,
            ff::TRACK_BAR_LARGE_STEP_DEFAULT,
            DesOmit::Eq(ff::TRACK_BAR_LARGE_STEP_DEFAULT),
        ),
    ),
    fp(
        ff::F_EXT_MARKING_STEP,
        "markingStep",
        "MarkingStep",
        Region::Ext,
        Codec::Int,
        keep(
            ff::TRACK_BAR_MARKING_STEP_DEFAULT,
            None,
            ff::TRACK_BAR_MARKING_STEP_DEFAULT,
            DesOmit::Eq(ff::TRACK_BAR_MARKING_STEP_DEFAULT),
        ),
    ),
    // orientation ПЕРЕД markingAppearance (witness Регулятор_Orientation_Vertical).
    fp(
        ff::F_EXT_ORIENTATION,
        "orientation",
        "Orientation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // markingAppearance: ПРОТИВОПОЛОЖНЫЕ дефолты (EDT опускает `DontShow`, Designer опускает
    // `BottomRight`; witness Регулятор_MarkingAppearance_НеПоказывать пуст в EDT).
    fp(
        ff::F_EXT_MARKING_APPEARANCE,
        "markingAppearance",
        "MarkingAppearance",
        Region::Ext,
        Codec::EnumTok,
        keep(
            ff::TRACK_BAR_MARKING_APPEARANCE_EDT_DEFAULT,
            Some(ff::TRACK_BAR_MARKING_APPEARANCE_EDT_DEFAULT),
            ff::TRACK_BAR_MARKING_APPEARANCE_DEFAULT,
            DesOmit::Eq(ff::TRACK_BAR_MARKING_APPEARANCE_DEFAULT),
        ),
    ),
];

/// extInfo ChartField (`form:ChartFieldExtInfo`): только геометрия. width/height — KEEP 50/10
/// (EDT всегда / Designer опускает дефолт); autoMax*/stretch — OppositeBool. Порядок = EDT.
/// Witness — ОценкаПроизводительности.ПодборЦелевогоВремениКлючевойОперации.
pub(crate) static CHART_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("50", None, "50", DesOmit::Eq("50"))),
    geo_auto_max_width(Policy::OppositeBool),
    geo_height(keep("10", None, "10", DesOmit::Eq("10"))),
    geo_auto_max_height(Policy::OppositeBool),
    // maxHeight — Symmetric Int (ERP 1⟷1; cf-ячейка НЕ витнесснута — типизированный отказ
    // cf-write; read-модель для тотальности designer-чтения ERP).
    geo_max_height(Policy::Symmetric),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
];

/// extInfo GanttChartField (`form:GanttChartFieldExtInfo`): та же геометрия, что у ChartField
/// (width/height — KEEP 50/10; autoMax*/stretch — OppositeBool). Авто-таблица (`<autoTable>` ⟷
/// `<Table>`) и событие `DetailProcessing` — СТРУКТУРНЫЕ поля узла (не в этой таблице), читаются/
/// пишутся спец-путём. Порядок = EDT-эмиссия (witness ДиспетчированиеГрафикаПроизводства.Планирование).
pub(crate) static GANTT_CHART_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("50", None, "50", DesOmit::Eq("50"))),
    geo_auto_max_width(Policy::OppositeBool),
    geo_height(keep("10", None, "10", DesOmit::Eq("10"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
];

/// extInfo PDFDocumentField (`form:PDFDocumentFieldExtInfo`): геометрия (width/height — KEEP
/// 50/10; autoMax*/stretch — OppositeBool) + `scale` (KEEP Int 100) + `currentPageNumber`
/// (KEEP Int 1) — оба EDT эмитит ВСЕГДА, Designer опускает дефолт (4/4 ERP-витнесса константны).
/// Добавление `viewStatusAddition` — структурное поле узла (item.additions), читается спец-путём.
/// Порядок = EDT-эмиссия (witness СервисДоставки.ПросмотрPDF).
pub(crate) static PDF_DOCUMENT_FIELD_EXT: &[FieldProj] = &[
    geo_width(keep("50", None, "50", DesOmit::Eq("50"))),
    geo_auto_max_width(Policy::OppositeBool),
    geo_height(keep("10", None, "10", DesOmit::Eq("10"))),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    fp(
        ff::F_EXT_SCALE,
        "scale",
        "Scale",
        Region::Ext,
        Codec::Int,
        keep("100", None, "100", DesOmit::Eq("100")),
    ),
    fp(
        ff::F_EXT_CURRENT_PAGE_NUMBER,
        "currentPageNumber",
        "CurrentPageNumber",
        Region::Ext,
        Codec::Int,
        keep("1", None, "1", DesOmit::Eq("1")),
    ),
];

/// extInfo GraphicalSchemaField (`form:FlowchartFieldExtInfo`): геометрия (width/height —
/// Symmetric; autoMax*/stretch — OppositeBool) + `edit` (OppositeBool: EDT опускает / Designer
/// эмитит `false`). Порядок = EDT (witness КартаМаршрутаБизнесПроцесса.Форма). Designer эмитит
/// лишь Width/Height/Edit (autoMax*/stretch дефолтны).
pub(crate) static FLOWCHART_FIELD_EXT: &[FieldProj] = &[
    geo_width(Policy::Symmetric),
    geo_auto_max_width(Policy::OppositeBool),
    geo_height(Policy::Symmetric),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    fp(
        ff::F_EXT_OUTPUT,
        "output",
        "Output",
        Region::Ext,
        Codec::EnumMap(&[("Enable", "Enable")]),
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_EDIT,
        "edit",
        "Edit",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
];

/// extInfo PeriodField (`form:PeriodFieldExtInfo`): autoMax*/stretch + `border` (BorderDef,
/// EDT всегда `Single` / Designer опускает). БЕЗ width/height. Порядок = EDT (Форма_ПоляВвода).
pub(crate) static PERIOD_FIELD_EXT: &[FieldProj] = &[
    geo_auto_max_width(Policy::OppositeBool),
    geo_auto_max_height(Policy::OppositeBool),
    geo_h_stretch(Policy::OppositeBool),
    geo_v_stretch(Policy::OppositeBool),
    fp(
        ff::F_EXT_BORDER,
        "border",
        "Border",
        Region::Ext,
        Codec::Border,
        keep(
            ff::BORDER_STYLE_SINGLE,
            None,
            ff::BORDER_STYLE_SINGLE,
            DesOmit::Eq(ff::BORDER_STYLE_SINGLE),
        ),
    ),
];
