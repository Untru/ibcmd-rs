//! Тела и extInfo Button / Decoration (Label+Picture) / Tooltip / FormCommand.

use crate::form::fields::{fp, Codec, DesOmit, FieldProj, Policy, Region};
use crate::form::tables::{fpa, keep, F_DEC_SHORTCUT, F_LD_BORDER_COLOR, KEEP_BOOL_TRUE};
use morph1c_core::spec::forms::command as fc;
use morph1c_core::spec::forms::controls::button as bt;
use morph1c_core::spec::forms::controls::form_field as ff;
use morph1c_core::spec::forms::controls::label_decoration as ld;

// ============================ Button: тело ============================

/// Свойства кнопки (порядок = канонический спек `button`). `font` НЕ смоделирован —
/// отсутствие в таблице даёт громкую §1.0-ошибку на обеих сторонах.
pub(crate) static BUTTON_BODY: &[FieldProj] = &[
    fpa(
        bt::F_DISPLAY_IMPORTANCE,
        "displayImportance",
        "DisplayImportance",
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        bt::F_TITLE,
        "title",
        "Title",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        bt::F_VISIBLE,
        "visible",
        "Visible",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        bt::F_ENABLED,
        "enabled",
        "Enabled",
        Region::Body,
        Codec::Bool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        bt::F_USER_VISIBLE,
        "userVisible",
        "UserVisible",
        Region::Body,
        Codec::CommonBool,
        KEEP_BOOL_TRUE,
    ),
    // dataPath данные-привязанной кнопки (SSL 52⟷52, Symmetric; метамодель Button #14).
    fp(
        bt::F_DATA_PATH,
        "dataPath",
        "DataPath",
        Region::Body,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    fp(
        bt::F_DEFAULT_ITEM,
        "defaultItem",
        "DefaultItem",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_SKIP_ON_INPUT,
        "skipOnInput",
        "SkipOnInput",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_TITLE_HEIGHT,
        "titleHeight",
        "TitleHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    // Вид кнопки: EDT опускает CommandBarButton; Designer эмитит ВСЕГДА (817/817).
    fp(
        bt::F_BUTTON_TYPE,
        "type",
        "Type",
        Region::Body,
        Codec::EnumTok,
        keep(
            bt::BUTTON_TYPE_EDT_DEFAULT,
            Some(bt::BUTTON_TYPE_EDT_DEFAULT),
            bt::BUTTON_TYPE_EDT_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        bt::F_COMMAND_NAME,
        "commandName",
        "CommandName",
        Region::Body,
        Codec::RefText,
        Policy::Symmetric,
    ),
    // parameter — командный параметр-ссылка (метамодель: commandName → parameter →
    // buttonImportance; witness Встреча/ЗапланированноеВзаимодействие/ТелефонныйЗвонок ×6).
    fp(
        bt::F_PARAMETER,
        "parameter",
        "Parameter",
        Region::Body,
        Codec::MdObjectRef,
        Policy::Symmetric,
    ),
    // buttonImportance: ТОЧНОЕ ПРИСУТСТВИЕ (fill'а нет НИ У ОДНОГО ридера — омиссии диалектов
    // НЕ дизъюнктны, ERP опускает тег в обоих 80 252/80 252). Ценз — `Policy::ButtonImportance`.
    fp(
        bt::F_BUTTON_IMPORTANCE,
        "buttonImportance",
        "ButtonImportance",
        Region::Body,
        Codec::EnumTok,
        Policy::ButtonImportance,
    ),
    fp(
        bt::F_REPRESENTATION,
        "representation",
        "Representation",
        Region::Body,
        Codec::EnumTok,
        keep(
            bt::REPRESENTATION_EDT_DEFAULT,
            Some(bt::REPRESENTATION_EDT_DEFAULT),
            bt::REPRESENTATION_DESIGNER_DEFAULT,
            DesOmit::Eq(bt::REPRESENTATION_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        bt::F_DEFAULT_BUTTON,
        "defaultButton",
        "DefaultButton",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_WIDTH,
        "width",
        "Width",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        bt::F_AUTO_MAX_WIDTH,
        "autoMaxWidth",
        "AutoMaxWidth",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // maxWidth — Int Symmetric, ПОСЛЕ autoMaxWidth, ДО height (метамодель Button; witness
    // УдалениеПомеченныхОбъектов кнопка-гиперссылка maxWidth=60).
    fp(
        bt::F_MAX_WIDTH,
        "maxWidth",
        "MaxWidth",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        bt::F_HEIGHT,
        "height",
        "Height",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        bt::F_AUTO_MAX_HEIGHT,
        "autoMaxHeight",
        "AutoMaxHeight",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // horizontalStretch ПЕРЕД groupHorizontalAlign (SSL EDT×2 + coverage EDT×21 + Designer×4, 0
    // контрпримеров при depth-точном разборе; напр. НастройкиТранспорта…НастройкаПодключенияКСервису).
    // Таблица — порядок эмиссии XML; cf-слот (button.rs) и drift-guard проверяют лишь МНОЖЕСТВО.
    fp(
        bt::F_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_GROUP_HORIZONTAL_ALIGN,
        "groupHorizontalAlign",
        "GroupHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        bt::F_MAX_HEIGHT,
        "maxHeight",
        "MaxHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        bt::F_VERTICAL_STRETCH,
        "verticalStretch",
        "VerticalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_GROUP_VERTICAL_ALIGN,
        "groupVerticalAlign",
        "GroupVerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // placementArea: EDT эмитит ВСЕГДА UserCmds (817/817); Designer не несёт никогда.
    fp(
        bt::F_PLACEMENT_AREA,
        "placementArea",
        "PlacementArea",
        Region::Body,
        Codec::EnumTok,
        keep(
            bt::PLACEMENT_AREA_FILL,
            None,
            bt::PLACEMENT_AREA_FILL,
            DesOmit::Always,
        ),
    ),
    fp(
        bt::F_CHECK,
        "check",
        "Check",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // Цвета/картинка — метамодель Button (textColor→backColor→borderColor→picture); корпус:
    // textColor<backColor×9, backColor<borderColor×9, borderColor<picture×1, backColor<picture×1;
    // 0 контрпримеров (прежняя позиция picture ДО цветов была слепой — ломала borderColor→picture).
    fp(
        bt::F_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        bt::F_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        bt::F_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        bt::F_PICTURE,
        "picture",
        "Picture",
        Region::Body,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        bt::F_TOOL_TIP_REPRESENTATION,
        "toolTipRepresentation",
        "ToolTipRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // representationInContextMenu: EDT эмитит ВСЕГДА (817/817); Designer опускает Auto.
    fp(
        bt::F_REPRESENTATION_IN_CONTEXT_MENU,
        "representationInContextMenu",
        "RepresentationInContextMenu",
        Region::Body,
        Codec::EnumTok,
        keep(
            bt::REPRESENTATION_IN_CONTEXT_MENU_EDT_DEFAULT,
            Some(bt::REPRESENTATION_IN_CONTEXT_MENU_EDT_DEFAULT),
            bt::REPRESENTATION_IN_CONTEXT_MENU_DEFAULT,
            DesOmit::Eq(bt::REPRESENTATION_IN_CONTEXT_MENU_DEFAULT),
        ),
    ),
    fp(
        bt::F_SHAPE,
        "shape",
        "Shape",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        bt::F_PICTURE_LOCATION,
        "pictureLocation",
        "PictureLocation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        bt::F_COMMAND_UNIQUENESS,
        "commandUniqueness",
        "CommandUniqueness",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_SHOW_AS_CARD,
        "showAsCard",
        "ShowAsCard",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        bt::F_SHAPE_REPRESENTATION,
        "shapeRepresentation",
        "ShapeRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        bt::F_LOCATION_IN_COMMAND_BAR,
        "locationInCommandBar",
        "LocationInCommandBar",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// Число головных полей кнопки, эмитимых EDT ДО `extendedTooltip`/`contextMenu`
/// (по `titleHeight` включительно; `dataPath` — в голове, метамодель Button 14 < 21).
pub(crate) const BUTTON_EDT_HEAD: usize = 9;

// ============================ Decoration: тело ============================

/// ОБЩЕЕ тело декораций Label/Picture (порядок = канонический спек `decoration_body`).
/// `title`/`formatted` — КАРКАС-glue (Designer несёт formatted АТРИБУТОМ на `<Title>`),
/// `font` НЕ смоделирован (§1.0-громко).
pub(crate) static DECORATION_BODY: &[FieldProj] = &[
    fpa(
        ld::F_DISPLAY_IMPORTANCE,
        "displayImportance",
        "DisplayImportance",
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ld::F_VISIBLE,
        "visible",
        "Visible",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ld::F_ENABLED,
        "enabled",
        "Enabled",
        Region::Body,
        Codec::Bool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        ld::F_USER_VISIBLE,
        "userVisible",
        "UserVisible",
        Region::Body,
        Codec::CommonBool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        ld::F_TOOL_TIP,
        "toolTip",
        "ToolTip",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ld::F_TOOL_TIP_REPRESENTATION,
        "toolTipRepresentation",
        "ToolTipRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // Геометрия декорации — порядок сверен topo-сортировкой Decoration-корпуса SSL (0 циклов):
    // width, autoMaxWidth, maxWidth, height, autoMaxHeight, horizontalStretch, verticalStretch,
    // groupHorizontalAlign, skipOnInput, groupVerticalAlign, textColor. Прежде maxWidth стоял
    // ПЕРЕД width/autoMaxWidth, а verticalStretch — в самом конце (после textColor); оба ломали
    // порядок (напр. РезультатыОбновленияПрограммы: autoMaxWidth<maxWidth; ЗаменаИОбъединение…:
    // verticalStretch<groupVerticalAlign).
    fp(
        ld::F_WIDTH,
        "width",
        "Width",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_AUTO_MAX_WIDTH,
        "autoMaxWidth",
        "AutoMaxWidth",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ld::F_MAX_WIDTH,
        "maxWidth",
        "MaxWidth",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_HEIGHT,
        "height",
        "Height",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_AUTO_MAX_HEIGHT,
        "autoMaxHeight",
        "AutoMaxHeight",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // maxHeight — Int Symmetric, ПОСЛЕ autoMaxHeight, ДО horizontalStretch (метамодель геометрии;
    // witness LabelDecoration НастройкиПользователей / PictureDecoration ПомощникИнтерактивного…).
    fp(
        ld::F_MAX_HEIGHT,
        "maxHeight",
        "MaxHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_VERTICAL_STRETCH,
        "verticalStretch",
        "VerticalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // Хвост = МЕТАМОДЕЛЬ Decoration (skipOnInput→textColor→[font-glue]→groupHorizontalAlign→
    // groupVerticalAlign; шрифт эмитится КОННЕКТОРОМ между textColor и groupHorizontalAlign —
    // см. DECORATION_EDT_FONT_SPLIT). Сверено SSL: skipOnInput<textColor×2, skipOnInput<GVA×5,
    // GHA<GVA×12, textColor<font×4, font<GHA×1, font<GVA×7; контрпримеров 0 (прежний порядок
    // GHA→skipOnInput→GVA→textColor был tie-break-слепым и не давал позиции для font-glue).
    fp(
        ld::F_SKIP_ON_INPUT,
        "skipOnInput",
        "SkipOnInput",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    // shortcut — ПОСЛЕ font-glue, ДО groupHorizontalAlign (метамодель Decoration; EDT witness
    // НастройкиОбменаФСС.ФормаЗаписи: autoMaxHeight→shortcut→groupVerticalAlign, т.е. в
    // ХВОСТЕ после [`DECORATION_EDT_FONT_SPLIT`]). Локальный id ERP-волны — вставка НЕ смещает
    // FONT_SPLIT (16), shortcut становится первой строкой пост-font-хвоста.
    fp(
        F_DEC_SHORTCUT,
        "shortcut",
        "Shortcut",
        Region::Body,
        Codec::Text,
        Policy::Symmetric,
    ),
    fp(
        ld::F_GROUP_HORIZONTAL_ALIGN,
        "groupHorizontalAlign",
        "GroupHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ld::F_GROUP_VERTICAL_ALIGN,
        "groupVerticalAlign",
        "GroupVerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// Число головных полей декорации, эмитимых EDT ДО стабов/`formatted`/`type`
/// (по `toolTipRepresentation` включительно; `title` — glue после `displayImportance`).
pub(crate) const DECORATION_EDT_HEAD: usize = 6;

/// Индекс первой строки ХВОСТА декорации, эмитимой EDT ПОСЛЕ `<font>`-glue (строка
/// `groupHorizontalAlign`; метамодель Decoration: textColor → font → shortcut →
/// groupHorizontalAlign). EDT-writer эмитит [HEAD..FONT_SPLIT], затем `<font>`, затем
/// [FONT_SPLIT..], затем extInfo.
pub(crate) const DECORATION_EDT_FONT_SPLIT: usize = 16;

/// extInfo LabelDecoration. Порядок = МЕТАМОДЕЛЬ `LabelDecorationExtInfo` (hyperlink,
/// horizontalAlign, verticalAlign, titleHeight, backColor, border) — сверено SSL:
/// hyperlink<horizontalAlign×76, horizontalAlign<verticalAlign×147, verticalAlign<border×2,
/// horizontalAlign<titleHeight×7/backColor×4/border×2; контрпримеров 0 (прежняя позиция
/// verticalAlign ПОСЛЕ border была слепой — ломала verticalAlign→border ПоискИУдалениеДублей).
pub(crate) static LABEL_DECORATION_EXT: &[FieldProj] = &[
    fp(
        ld::F_EXT_HYPERLINK,
        "hyperlink",
        "Hyperlink",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // horizontalAlign: ПРОТИВОПОЛОЖНЫЕ дефолты (EDT опускает `Auto`, Designer опускает `Left`;
    // сверено корпусом покрытия — надпись `ГоризонтальноеПоложение_Авто` пуста в EDT).
    fp(
        ld::F_EXT_HORIZONTAL_ALIGN,
        "horizontalAlign",
        "HorizontalAlign",
        Region::Ext,
        Codec::EnumTok,
        keep(
            ld::HORIZONTAL_ALIGN_EDT_DEFAULT,
            Some(ld::HORIZONTAL_ALIGN_EDT_DEFAULT),
            ld::HORIZONTAL_ALIGN_LEFT,
            DesOmit::Eq(ld::HORIZONTAL_ALIGN_LEFT),
        ),
    ),
    fp(
        ld::F_EXT_VERTICAL_ALIGN,
        "verticalAlign",
        "VerticalAlign",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_TITLE_HEIGHT,
        "titleHeight",
        "TitleHeight",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // borderColor — Color Symmetric (метамодель LabelDecorationExtInfo#6; ERP ×14).
    // cf: LabelDecoration-ext `{5,…}` ячейка [7] (абляция s10).
    fp(
        F_LD_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // border: канон-стиль-Enum (`Overline`/`Single`/`WithoutBorder@3`); EDT/Designer опускают
    // дефолт `WithoutBorder` (ширины 1), эмитят прочее (witnesses ПанельОтчетов `Overline`,
    // ПоискИУдалениеДублей `Single`, НастройкиРаботыСФайловымАрхивом `WithoutBorder@3`).
    fp(
        ld::F_EXT_BORDER,
        "border",
        "Border",
        Region::Ext,
        Codec::Border,
        keep(
            ff::BORDER_STYLE_WITHOUT,
            Some(ff::BORDER_STYLE_WITHOUT),
            ff::BORDER_STYLE_WITHOUT,
            DesOmit::Eq(ff::BORDER_STYLE_WITHOUT),
        ),
    ),
];

/// extInfo PictureDecoration.
pub(crate) static PICTURE_DECORATION_EXT: &[FieldProj] = &[
    fp(
        ld::F_EXT_PICTURE,
        "picture",
        "Picture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_HYPERLINK,
        "hyperlink",
        "Hyperlink",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_PICTURE_SIZE,
        "pictureSize",
        "PictureSize",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_PICTURE_COLOR,
        "pictureColor",
        "PictureColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_ZOOMABLE,
        "zoomable",
        "Zoomable",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_ENABLE_START_DRAG,
        "enableStartDrag",
        "EnableStartDrag",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_ENABLE_DRAG,
        "enableDrag",
        "EnableDrag",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_PIC_BORDER,
        "border",
        "Border",
        Region::Ext,
        Codec::Border,
        keep(
            ff::BORDER_STYLE_WITHOUT,
            Some(ff::BORDER_STYLE_WITHOUT),
            ff::BORDER_STYLE_WITHOUT,
            DesOmit::Eq(ff::BORDER_STYLE_WITHOUT),
        ),
    ),
    // borderColor — СРАЗУ за border (EDT witness Мастер_ПаспортныеДанные Силуэт: border→
    // borderColor; Designer — BorderColor ПЕРЕД Border, см. DES_PICTURE_DECORATION_ORDER). Общий
    // локальный id [`F_LD_BORDER_COLOR`] (декорации-borderColor), Color-кодек. cf — НЕ витнесснут
    // (SSL PictureDecoration без borderColor) ⇒ cf-writer отвергает типизированно.
    fp(
        F_LD_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // imageScale ПЕРЕД fileDragMode (SSL EDT PictureDecoration: imageScale<fileDragMode; напр.
    // ТранспортСообщенийОбменаПассивныйРежим.ФормаНастройки). Designer-порядок (DES) уже верен.
    fp(
        ld::F_EXT_IMAGE_SCALE,
        "imageScale",
        "ImageScale",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    // nonselectedPictureText — Localized Symmetric (метамодель imageScale→nonselectedPictureText;
    // witness УправлениеПодключениемDSS: EDT picture→nonselectedPictureText→fileDragMode).
    fp(
        ld::F_EXT_NONSELECTED_PICTURE_TEXT,
        "nonselectedPictureText",
        "NonselectedPictureText",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ld::F_EXT_FILE_DRAG_MODE,
        "fileDragMode",
        "FileDragMode",
        Region::Ext,
        Codec::EnumTok,
        keep(
            ld::FILE_DRAG_MODE_EDT_DEFAULT,
            Some(ld::FILE_DRAG_MODE_EDT_DEFAULT),
            ld::FILE_DRAG_MODE_DESIGNER_DEFAULT,
            DesOmit::Eq(ld::FILE_DRAG_MODE_DESIGNER_DEFAULT),
        ),
    ),
];

// ============================ Tooltip: тело (standalone) ============================

/// Тело расширенной подсказки (`<extendedTooltip>`/`<ExtendedTooltip>`) — вложенный
/// `LabelDecoration`-стаб. STANDALONE-срез (НЕ в drift-guard `projection_tables_match_control_specs`,
/// который проверяет лишь таблицы контролов): подмножество геометрии декорации + `textColor`.
/// `title`/`formatted`/`type=Label`/extInfo(`horizontalAlign`/`verticalAlign`)/events — КАРКАС-glue
/// (читаются/пишутся коннектором: EDT синтезирует `type=Label`+`form:LabelDecorationExtInfo`, Designer
/// несёт `formatted` АТРИБУТОМ на `<Title>`). `F_HORIZONTAL_STRETCH` (FieldId 8) НЕ входит в
/// `label_decoration().properties`, поэтому телу нужен СОБСТВЕННЫЙ срез. Порядок строк = канон-порядок
/// тела подсказки, общий обоим форматам (⇒ X-структурность) — совпадает с геометрией [`DECORATION_BODY`].
pub(crate) static TOOLTIP_BODY: &[FieldProj] = &[
    fp(
        ld::F_WIDTH,
        "width",
        "Width",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_AUTO_MAX_WIDTH,
        "autoMaxWidth",
        "AutoMaxWidth",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ld::F_MAX_WIDTH,
        "maxWidth",
        "MaxWidth",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_HEIGHT,
        "height",
        "Height",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_AUTO_MAX_HEIGHT,
        "autoMaxHeight",
        "AutoMaxHeight",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // maxHeight — Symmetric Int (ERP-witness подсказки ×2; cf tooltip flat[29] — абляция s10).
    fp(
        ld::F_MAX_HEIGHT,
        "maxHeight",
        "MaxHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ld::F_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // verticalStretch — Symmetric Bool (ERP-witness подсказки ×3; cf tooltip flat[13]
    // тристейт — абляция s10 2→1).
    fp(
        ld::F_VERTICAL_STRETCH,
        "verticalStretch",
        "VerticalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ld::F_GROUP_HORIZONTAL_ALIGN,
        "groupHorizontalAlign",
        "GroupHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ld::F_GROUP_VERTICAL_ALIGN,
        "groupVerticalAlign",
        "GroupVerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ld::F_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
];

// ============================ FormCommand: плоское тело (standalone) ============================

/// ПЛОСКИЕ поля пользовательской команды формы (`<formCommands>`/`<Command>`), драйвимые
/// табличным движком. STANDALONE-срез (спек — `spec/forms/command`, НЕ control_spec_for ⇒ вне
/// drift-guard). `name`/`id`/`use`(EDT-константа)/`picture`(PictureRef+суффикс-суппресс)/`action`
/// (EDT handler-контейнер ⟺ Designer текст)/`functionalOptions`(repeatable/контейнер)/
/// `associatedTableElementId`(StringValue/xs:string обёртка) — КАРКАС-glue (несовместимы с
/// FieldProj-кодеками). `modifiesStoredData`⟷`ModifiesSavedData` — РАЗНОИМЁННЫЕ теги (edt≠des).
/// Порядок строк несуществен для READ (коннектор сортирует `sort_props_by_spec`).
pub(crate) static COMMAND_BODY: &[FieldProj] = &[
    fp(
        fc::F_TITLE,
        "title",
        "Title",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        fc::F_TOOL_TIP,
        "toolTip",
        "ToolTip",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        fc::F_SHORTCUT,
        "shortcut",
        "Shortcut",
        Region::Body,
        Codec::Text,
        Policy::Symmetric,
    ),
    fp(
        fc::F_ACTION_PURPOSE,
        "actionPurpose",
        "ActionPurpose",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fc::F_REPRESENTATION,
        "representation",
        "Representation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fc::F_MODIFIES_STORED_DATA,
        "modifiesStoredData",
        "ModifiesSavedData",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // currentRowUse/selectedRowsUse: ПРОТИВОПОЛОЖНЫЕ пер-форматные дефолты (EDT опускает `Use`,
    // Designer опускает `Auto`) — required, канон-bag несёт всегда (Keep: каждый ридер заполняет
    // свой дефолт при отсутствии, каждый writer опускает == своему дефолту).
    fp(
        fc::F_CURRENT_ROW_USE,
        "currentRowUse",
        "CurrentRowUse",
        Region::Body,
        Codec::EnumTok,
        keep(
            fc::ROW_USE_EDT_DEFAULT,
            Some(fc::ROW_USE_EDT_DEFAULT),
            fc::ROW_USE_DESIGNER_DEFAULT,
            DesOmit::Eq(fc::ROW_USE_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        fc::F_SELECTED_ROWS_USE,
        "selectedRowsUse",
        "SelectedRowsUse",
        Region::Body,
        Codec::EnumTok,
        keep(
            fc::ROW_USE_EDT_DEFAULT,
            Some(fc::ROW_USE_EDT_DEFAULT),
            fc::ROW_USE_DESIGNER_DEFAULT,
            DesOmit::Eq(fc::ROW_USE_DESIGNER_DEFAULT),
        ),
    ),
];

