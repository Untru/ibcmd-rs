//! Table: полное тело + HEAD-маркеры, DynamicListTableExtInfo, Designer-порядки Таблицы.

use crate::form::fields::{fp, Codec, DesOmit, FieldProj, Policy, Region};
use crate::form::tables::{
    fpa, keep, DesSlot, F_TB_BEHAVIOR_ON_HORIZONTAL_COMPRESSION, F_TB_REFRESH_REQUEST,
    F_TB_SHORTCUT, F_TB_TITLE_HEIGHT, F_TB_USE_ALTERNATION_ROW_COLOR, KEEP_BOOL_TRUE,
};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::forms::controls::table as tb;

// ============================ Table: тело (LANE-F-4) ============================

/// ПОЛНОЕ тело Таблицы (порядок = канонический спек = EDT-порядок эмиссии). Пер-форматные
/// дефолты — cross-omission-свидетели корпуса (см. `core/spec/forms/controls/table`).
/// `displayImportance` — Designer-АТРИБУТ. Table extInfo-обёртки НЕ несёт (поля инлайн).
pub(crate) static TABLE_BODY: &[FieldProj] = &[
    // -- HEAD_A (до excludedCommands/колонок) [0..10] --
    fpa(
        tb::F_DISPLAY_IMPORTANCE,
        "displayImportance",
        "DisplayImportance",
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_TITLE,
        "title",
        "Title",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    // titleTextColor — Color Symmetric, СРАЗУ после title (метамодель Table; witness
    // СопоставлениеОбъектовИнформационныхБаз: EDT title→titleTextColor→visible).
    fp(
        tb::F_TITLE_TEXT_COLOR,
        "titleTextColor",
        "TitleTextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        tb::F_VISIBLE,
        "visible",
        "Visible",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_ENABLED,
        "enabled",
        "Enabled",
        Region::Body,
        Codec::Bool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        tb::F_USER_VISIBLE,
        "userVisible",
        "UserVisible",
        Region::Body,
        Codec::CommonBool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        tb::F_DATA_PATH,
        "dataPath",
        "DataPath",
        Region::Body,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    fp(
        tb::F_DEFAULT_ITEM,
        "defaultItem",
        "DefaultItem",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_SKIP_ON_INPUT,
        "skipOnInput",
        "SkipOnInput",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_TITLE_LOCATION,
        "titleLocation",
        "TitleLocation",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::TITLE_LOCATION_EDT_DEFAULT,
            Some(tb::TITLE_LOCATION_EDT_DEFAULT),
            tb::TITLE_LOCATION_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::TITLE_LOCATION_DESIGNER_DEFAULT),
        ),
    ),
    // titleHeight/shortcut — Symmetric (метамодель Table#20/#21: titleLocation→titleHeight→
    // shortcut; ERP 7⟷(в общем счёте 790 c полями) и 3⟷3). cf: HEAD[7] и HEAD[51]
    // (абляция s10: 0→2; {0,0,0}→{0,49,8} для Ctrl+1).
    fp(
        F_TB_TITLE_HEIGHT,
        "titleHeight",
        "TitleHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        F_TB_SHORTCUT,
        "shortcut",
        "Shortcut",
        Region::Body,
        Codec::Text,
        Policy::Symmetric,
    ),
    // -- HEAD_MID (ПОСЛЕ excludedCommands, ДО колонок) [10..12] --
    // `toolTip`/`toolTipRepresentation` — метамодель `excludedCommands, toolTip, toolTipRepresentation,
    // items`; witness Календари/ШаблонЗаполнения (без excludedCommands) и ГруппыДоступа (с
    // excludedCommands) — оба несут toolTip ПОСЛЕ excludedCommands, ДО колонок.
    fp(
        tb::F_TOOL_TIP,
        "toolTip",
        "ToolTip",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        tb::F_TOOL_TIP_REPRESENTATION,
        "toolTipRepresentation",
        "ToolTipRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // -- HEAD_B (после колонок, до autoCommandBar) [12..13] --
    fp(
        tb::F_COMMAND_BAR_LOCATION,
        "commandBarLocation",
        "CommandBarLocation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // -- TAIL (после decorator-стабов) [13..] --
    fp(
        tb::F_REPRESENTATION,
        "representation",
        "Representation",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::REPRESENTATION_EDT_DEFAULT,
            Some(tb::REPRESENTATION_EDT_DEFAULT),
            tb::REPRESENTATION_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::REPRESENTATION_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        tb::F_AUTO_FILL,
        "autoFill",
        "Autofill",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_READ_ONLY,
        "readOnly",
        "ReadOnly",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_CHANGE_ROW_SET,
        "changeRowSet",
        "ChangeRowSet",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_CHANGE_ROW_ORDER,
        "changeRowOrder",
        "ChangeRowOrder",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_WIDTH,
        "width",
        "Width",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_MAX_WIDTH,
        "autoMaxWidth",
        "AutoMaxWidth",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // maxWidth — Int Symmetric, ПОСЛЕ autoMaxWidth, ДО height (метамодель Table; witness
    // ПоискИУдалениеДублей.ПравилаПоиска maxWidth=60).
    fp(
        tb::F_MAX_WIDTH,
        "maxWidth",
        "MaxWidth",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        tb::F_HEIGHT,
        "height",
        "Height",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_MAX_HEIGHT,
        "autoMaxHeight",
        "AutoMaxHeight",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_MAX_HEIGHT,
        "maxHeight",
        "MaxHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        tb::F_HEIGHT_IN_TABLE_ROWS,
        "heightInTableRows",
        "HeightInTableRows",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        tb::F_HEIGHT_CONTROL_VARIANT,
        "heightControlVariant",
        "HeightControlVariant",
        Region::Body,
        Codec::EnumMap(&[
            ("InFormRows", "UseHeightInFormRows"),
            ("ByContent", "UseContentHeight"),
            ("InTableRows", "UseHeightInTableRows"),
        ]),
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_MAX_ROWS_COUNT,
        "autoMaxRowsCount",
        "AutoMaxRowsCount",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // maxRowsCount — МЕТАМОДЕЛЬ Table: heightControlVariant → autoMaxRowsCount →
    // **maxRowsCount** → choiceMode (SSL: heightControlVariant<maxRowsCount×13,
    // maxRowsCount<selectionMode×6/header×4; прежняя позиция после header ломала
    // МашиночитаемыеДоверенности — вскрыто после снятия read-блокера).
    fp(
        tb::F_MAX_ROWS_COUNT,
        "maxRowsCount",
        "MaxRowsCount",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    // choiceMode ПЕРЕД multipleChoice (SSL EDT×6 и Designer×6, 0 контрпримеров; напр.
    // ЯзыкиПечатныхФорм.ПодборЯзыкаИзСпискаДоступных).
    fp(
        tb::F_CHOICE_MODE,
        "choiceMode",
        "ChoiceMode",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_MULTIPLE_CHOICE,
        "multipleChoice",
        "MultipleChoice",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_ROW_INPUT_MODE,
        "rowInputMode",
        "RowInputMode",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_SELECTION_MODE,
        "selectionMode",
        "SelectionMode",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::SELECTION_MODE_EDT_DEFAULT,
            Some(tb::SELECTION_MODE_EDT_DEFAULT),
            tb::SELECTION_MODE_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::SELECTION_MODE_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        tb::F_ROW_SELECTION_MODE,
        "rowSelectionMode",
        "RowSelectionMode",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::ROW_SELECTION_MODE_EDT_DEFAULT,
            Some(tb::ROW_SELECTION_MODE_EDT_DEFAULT),
            tb::ROW_SELECTION_MODE_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::ROW_SELECTION_MODE_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        tb::F_ROW_ACTIONS_SHOW_TYPE,
        "rowActionsShowType",
        "RowActionsShowType",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_HEADER,
        "header",
        "Header",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // headerHeight: EDT эмитит всегда; Designer НЕСЁТ не-дефолт (напр. `2` в списковых формах SSL),
    // опускает лишь дефолт `1` (KEEP Eq, как footerHeight — не Always: иначе Designer теряет значение).
    fp(
        tb::F_HEADER_HEIGHT,
        "headerHeight",
        "HeaderHeight",
        Region::Body,
        Codec::Int,
        keep(
            tb::HEADER_HEIGHT_FILL,
            None,
            tb::HEADER_HEIGHT_FILL,
            DesOmit::Eq(tb::HEADER_HEIGHT_FILL),
        ),
    ),
    fp(
        tb::F_FOOTER,
        "footer",
        "Footer",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_FOOTER_HEIGHT,
        "footerHeight",
        "FooterHeight",
        Region::Body,
        Codec::Int,
        keep(
            tb::FOOTER_HEIGHT_EDT_DEFAULT,
            Some(tb::FOOTER_HEIGHT_EDT_DEFAULT),
            tb::FOOTER_HEIGHT_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::FOOTER_HEIGHT_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        tb::F_HORIZONTAL_SCROLL_BAR,
        "horizontalScrollBar",
        "HorizontalScrollBar",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::HORIZONTAL_SCROLL_BAR_EDT_DEFAULT,
            Some(tb::HORIZONTAL_SCROLL_BAR_EDT_DEFAULT),
            tb::SCROLL_BAR_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::SCROLL_BAR_DESIGNER_DEFAULT),
        ),
    ),
    // verticalScrollBar: те же ПРОТИВОПОЛОЖНЫЕ дефолты, что horizontalScrollBar (EDT опускает
    // `DontUse`, Designer опускает `AutoUse`; сверено корпусом покрытия).
    fp(
        tb::F_VERTICAL_SCROLL_BAR,
        "verticalScrollBar",
        "VerticalScrollBar",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::HORIZONTAL_SCROLL_BAR_EDT_DEFAULT,
            Some(tb::HORIZONTAL_SCROLL_BAR_EDT_DEFAULT),
            tb::SCROLL_BAR_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::SCROLL_BAR_DESIGNER_DEFAULT),
        ),
    ),
    // horizontalLines / verticalLines — SYMMETRIC (both formats emit the value as-is; absent ⇒ not
    // in bag). The cf `{73,…}` Table needs the full TRISTATE (specified-false → TAIL[37/38]=0 +
    // HEAD[32]=0, specified-true → 1/1, unspecified → 2), which the prior `OppositeBool` collapsed
    // (explicit-false dropped, Designer-absent forced to `true`) — losing exactly the distinction the
    // composer `*AvailableFields` tables carry (`<HorizontalLines>false</HorizontalLines>` → cf 0, not
    // 2). Witnessed on the DeepHarness composer synthetic (b2). SSL edt→cf stays byte-exact because
    // its Table lines ride the separate `…BWA` twins (37/38), so field 35/36 is absent there.
    fp(
        tb::F_HORIZONTAL_LINES,
        "horizontalLines",
        "HorizontalLines",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_VERTICAL_LINES,
        "verticalLines",
        "VerticalLines",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // useAlternationRowColor — ПЛОСКИЙ Symmetric Bool (метамодель Table#73, verticalLines→
    // useAlternationRowColor→…BWA-твины; ERP 3864⟷3864×true; ОТДЕЛЬНОЕ поле от нуляемого
    // useAlternationRowColorBWA ниже). cf: TAIL[47] + дериват HEAD[36] (абляция s10).
    fp(
        F_TB_USE_ALTERNATION_ROW_COLOR,
        "useAlternationRowColor",
        "UseAlternationRowColor",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_HORIZONTAL_LINES_BWA,
        "horizontalLinesBWA",
        "HorizontalLinesBWA",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_VERTICAL_LINES_BWA,
        "verticalLinesBWA",
        "VerticalLinesBWA",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_USE_ALTERNATION_ROW_COLOR_BWA,
        "useAlternationRowColorBWA",
        "UseAlternationRowColorBWA",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_INSERT_NEW_ROW,
        "autoInsertNewRow",
        "AutoInsertNewRow",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_ADD_INCOMPLETE,
        "autoAddIncomplete",
        "AutoAddIncomplete",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_MARK_INCOMPLETE,
        "autoMarkIncomplete",
        "AutoMarkIncomplete",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // searchOnInput/initialListView: ПРОТИВОПОЛОЖНЫЕ дефолты (EDT омитит `Use`/`Beginning`,
    // Designer омитит `Auto`; сверено корпусом покрытия).
    fp(
        tb::F_SEARCH_ON_INPUT,
        "searchOnInput",
        "SearchOnInput",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::SEARCH_ON_INPUT_EDT_DEFAULT,
            Some(tb::SEARCH_ON_INPUT_EDT_DEFAULT),
            tb::AUTO_DEFAULT,
            DesOmit::Eq(tb::AUTO_DEFAULT),
        ),
    ),
    fp(
        tb::F_INITIAL_LIST_VIEW,
        "initialListView",
        "InitialListView",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::INITIAL_LIST_VIEW_EDT_DEFAULT,
            Some(tb::INITIAL_LIST_VIEW_EDT_DEFAULT),
            tb::AUTO_DEFAULT,
            DesOmit::Eq(tb::AUTO_DEFAULT),
        ),
    ),
    fp(
        tb::F_INITIAL_TREE_VIEW,
        "initialTreeView",
        "InitialTreeView",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_INITIAL_ROW_ACTIVATION,
        "initialRowActivation",
        "InitialRowActivation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_OUTPUT,
        "output",
        "Output",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_VERTICAL_STRETCH,
        "verticalStretch",
        "VerticalStretch",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_ENABLE_START_DRAG,
        "enableStartDrag",
        "EnableStartDrag",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_ENABLE_DRAG,
        "enableDrag",
        "EnableDrag",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        tb::F_FILE_DRAG_MODE,
        "fileDragMode",
        "FileDragMode",
        Region::Body,
        Codec::EnumTok,
        keep(
            tb::FILE_DRAG_MODE_EDT_DEFAULT,
            Some(tb::FILE_DRAG_MODE_EDT_DEFAULT),
            tb::FILE_DRAG_MODE_DESIGNER_DEFAULT,
            DesOmit::Eq(tb::FILE_DRAG_MODE_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        tb::F_ROW_PICTURE_DATA_PATH,
        "rowPictureDataPath",
        "RowPictureDataPath",
        Region::Body,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    fp(
        tb::F_ROWS_PICTURE,
        "rowsPicture",
        "RowsPicture",
        Region::Body,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        tb::F_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        tb::F_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        tb::F_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        tb::F_CELL_HYPERLINKS_REPRESENTATION,
        "cellHyperlinksRepresentation",
        "CellHyperlinksRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_SEARCH_STRING_LOCATION,
        "searchStringLocation",
        "SearchStringLocation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_VIEW_STATUS_LOCATION,
        "viewStatusLocation",
        "ViewStatusLocation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_SEARCH_CONTROL_LOCATION,
        "searchControlLocation",
        "SearchControlLocation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_GROUP_HORIZONTAL_ALIGN,
        "groupHorizontalAlign",
        "GroupHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_GROUP_VERTICAL_ALIGN,
        "groupVerticalAlign",
        "GroupVerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // refreshRequest — Symmetric Enum (метамодель Table#109: groupVerticalAlign→refreshRequest→
    // currentRowUse; ERP 38⟷38×PullFromTop). cf: TAIL[21] absent 0 / PullFromTop 1 (абляция s10).
    fp(
        F_TB_REFRESH_REQUEST,
        "refreshRequest",
        "RefreshRequest",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_CURRENT_ROW_USE,
        "currentRowUse",
        "CurrentRowUse",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // behaviorOnHorizontalCompression — Symmetric Enum (метамодель Table#111; ERP 1⟷1×
    // MoveItemsByImportance). cf: TAIL[33] absent 0 / MoveItemsByImportance 2 (абляция s10).
    fp(
        F_TB_BEHAVIOR_ON_HORIZONTAL_COMPRESSION,
        "behaviorOnHorizontalCompression",
        "BehaviorOnHorizontalCompression",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_AUTO_MAX_CARD_HEIGHT,
        "autoMaxCardHeight",
        "AutoMaxCardHeight",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        tb::F_ROW_FILTER,
        "rowFilter",
        "RowFilter",
        Region::Body,
        Codec::UndefinedValue,
        Policy::Symmetric,
    ),
    fp(
        tb::F_VIEW_MODE,
        "viewMode",
        "ViewMode",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // complexSettingsViewMode — сразу после viewMode (witness РассылкиОтчетов viewMode→complexSettingsViewMode).
    fp(
        tb::F_COMPLEX_SETTINGS_VIEW_MODE,
        "complexSettingsViewMode",
        "ComplexSettingsViewMode",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        tb::F_SETTINGS_NAMED_ITEM_DETAILED_REPRESENTATION,
        "settingsNamedItemDetailedRepresentation",
        "SettingsNamedItemDetailedRepresentation",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
];

/// Число HEAD_A-полей Таблицы (эмитятся ДО excludedCommands): по `shortcut` вкл.
/// (ERP-волна: +2 строки `titleHeight`/`shortcut` после `titleLocation` — метамодель
/// Table#20/#21, регион ДО excludedCommands(25)).
pub(crate) const TABLE_HEAD_A: usize = 12;
/// Конец HEAD_MID-полей Таблицы (эмитятся ПОСЛЕ excludedCommands, ДО колонок): `toolTip`,
/// `toolTipRepresentation` (метамодель `excludedCommands, toolTip, toolTipRepresentation, items`).
pub(crate) const TABLE_HEAD_MID: usize = 14;
/// Конец HEAD_B-полей Таблицы (эмитятся после колонок, ДО autoCommandBar): `commandBarLocation`
/// (метамодель после `items`); далее — TAIL (после decorator-стабов).
pub(crate) const TABLE_HEAD_B: usize = 15;

/// Designer-порядок эмиссии Таблицы (topo по корпусу; поля-атрибуты `DisplayImportance`
/// эмитятся отдельно). Слот-маркеры: `TableCommandSet`/`TableAutoCommandBar`/`TableAdditions`/
/// `ContextMenu`/`ExtendedTooltip`/`Events`/`ChildItems`.
pub(crate) static DES_TABLE_ORDER: &[DesSlot] = &[
    DesSlot::F(tb::F_REPRESENTATION),
    // titleLocation — СРАЗУ после Representation, ДО CommandBarLocation/ReadOnly (SSL Designer:
    // Representation<TitleLocation×46, TitleLocation<CommandBarLocation×13, TitleLocation<
    // ReadOnly×9, 0 контрпримеров; напр. ПроизводственныеКалендари.ФормаЭлемента). Прежняя
    // позиция после ReadOnly ломала Representation→TitleLocation→CommandBarLocation.
    DesSlot::F(tb::F_TITLE_LOCATION),
    // userVisible/visible/enabled — МЕЖДУ TitleLocation и CommandBarLocation (SSL:
    // Representation<Visible×4/UserVisible×1, Visible<CommandBarLocation×1, Visible<
    // Autofill×1 — ИнтерактивноеИзменениеВыгрузки.СоставВыгрузки; 0 контрпримеров).
    // Прежняя позиция перед DataPath ломала Visible→Autofill.
    DesSlot::F(tb::F_USER_VISIBLE),
    DesSlot::F(tb::F_VISIBLE),
    DesSlot::F(tb::F_ENABLED),
    DesSlot::F(tb::F_COMMAND_BAR_LOCATION),
    DesSlot::F(tb::F_AUTO_FILL),
    DesSlot::F(tb::F_READ_ONLY),
    DesSlot::F(tb::F_SKIP_ON_INPUT),
    DesSlot::F(tb::F_DEFAULT_ITEM),
    DesSlot::F(tb::F_CHANGE_ROW_SET),
    DesSlot::F(tb::F_CHANGE_ROW_ORDER),
    // Геометрия — ШИРИНА раньше ВЫСОТЫ (SSL Designer: Width<AutoMaxWidth×7, AutoMaxWidth<
    // MaxWidth×5, AutoMaxWidth<Height×4/AutoMaxHeight×5/MaxHeight×1, MaxWidth<Height×1;
    // Height<Width НЕ витнессирован — прежний высота-первой порядок ломал
    // AutoMaxWidth→Height, напр. ГрупповоеИзменениеРеквизитов.Форма). maxWidth — после
    // Width/AutoMaxWidth, ДО SelectionMode (witness ДополнительныеРеквизитыИСведения.
    // ЗависимостьРеквизитов: Width→MaxWidth→SelectionMode).
    DesSlot::F(tb::F_WIDTH),
    DesSlot::F(tb::F_AUTO_MAX_WIDTH),
    DesSlot::F(tb::F_MAX_WIDTH),
    DesSlot::F(tb::F_HEIGHT),
    DesSlot::F(tb::F_AUTO_MAX_HEIGHT),
    DesSlot::F(tb::F_MAX_HEIGHT),
    DesSlot::F(tb::F_HEIGHT_IN_TABLE_ROWS),
    DesSlot::F(tb::F_HEIGHT_CONTROL_VARIANT),
    DesSlot::F(tb::F_CHOICE_MODE),
    DesSlot::F(tb::F_MULTIPLE_CHOICE),
    DesSlot::F(tb::F_ROW_INPUT_MODE),
    DesSlot::F(tb::F_AUTO_MAX_ROWS_COUNT),
    DesSlot::F(tb::F_MAX_ROWS_COUNT),
    DesSlot::F(tb::F_SELECTION_MODE),
    DesSlot::F(tb::F_ROW_SELECTION_MODE),
    DesSlot::F(tb::F_HEADER),
    // headerHeight — Designer эмитит СРАЗУ после Header (witness DSS.ПодборУчетнойЗаписи:
    // RowSelectionMode→Header→HeaderHeight; списковые формы SSL без Header дают
    // RowSelectionMode→HeaderHeight). Опускается лишь при дефолте `1` (KEEP Eq).
    DesSlot::F(tb::F_HEADER_HEIGHT),
    DesSlot::F(tb::F_FOOTER),
    DesSlot::F(tb::F_FOOTER_HEIGHT),
    DesSlot::F(tb::F_HORIZONTAL_SCROLL_BAR),
    DesSlot::F(tb::F_VERTICAL_SCROLL_BAR),
    DesSlot::F(tb::F_HORIZONTAL_LINES),
    DesSlot::F(tb::F_VERTICAL_LINES),
    // ПЛОСКИЙ UseAlternationRowColor — В ТОМ ЖЕ слоте, что его 2.21-твин ниже: сразу после
    // пары линий. Топо-перепись по 3 872 таблицам, несущим этот тег (ERP 2.20 + дамп 2.17
    // `integration_subsystem`), даёт ОДНОЗНАЧНЫЙ порядок без единого контрпримера:
    // HorizontalLines×178 / VerticalLines×169 / VerticalScrollBar×29 / RowSelectionMode×111
    // — ДО, а AutoInsertNewRow×350 / InitialTreeView×2433 / SearchOnInput×42 — ПОСЛЕ.
    // Прежняя позиция (сразу после DefaultItem) опиралась на ERP-формы, где промежуточных
    // тегов НЕТ вовсе (`Задача.ФормаСписка`: DefaultItem→UseAlternationRowColor→
    // InitialTreeView) — она этому витнессу не противоречит, но и не различает слот;
    // 2.17-дамп различает (`CommonForms/конс_Просмотрщик`: HorizontalLines→
    // UseAlternationRowColor→AutoInsertNewRow). В Designer плоское имя и `…BWA` НИКОГДА не
    // сосуществуют (2.20 — только плоское, 2.21 — только BWA), так что соседство безопасно.
    DesSlot::F(F_TB_USE_ALTERNATION_ROW_COLOR),
    DesSlot::F(tb::F_HORIZONTAL_LINES_BWA),
    DesSlot::F(tb::F_VERTICAL_LINES_BWA),
    DesSlot::F(tb::F_USE_ALTERNATION_ROW_COLOR_BWA),
    DesSlot::F(tb::F_AUTO_INSERT_NEW_ROW),
    DesSlot::F(tb::F_AUTO_ADD_INCOMPLETE),
    DesSlot::F(tb::F_AUTO_MARK_INCOMPLETE),
    DesSlot::F(tb::F_SEARCH_ON_INPUT),
    DesSlot::F(tb::F_INITIAL_LIST_VIEW),
    DesSlot::F(tb::F_INITIAL_TREE_VIEW),
    DesSlot::F(tb::F_INITIAL_ROW_ACTIVATION),
    // RowActionsShowType ПЕРЕД Output (единственный ко-витнесс: ЗащитаПерсональныхДанных
    // InitialListView→RowActionsShowType→Output; прежний порядок был tie-break-слепым).
    DesSlot::F(tb::F_ROW_ACTIONS_SHOW_TYPE),
    DesSlot::F(tb::F_OUTPUT),
    // HorizontalStretch ПЕРЕД VerticalStretch (SSL Designer Table: H<V×5, 0 контрпримеров;
    // прежний обратный порядок вскрыт МашиночитаемыеДоверенности.ФормаЭлемента).
    DesSlot::F(tb::F_HORIZONTAL_STRETCH),
    DesSlot::F(tb::F_VERTICAL_STRETCH),
    DesSlot::F(tb::F_ENABLE_START_DRAG),
    DesSlot::F(tb::F_ENABLE_DRAG),
    DesSlot::F(tb::F_FILE_DRAG_MODE),
    // visible/enabled/userVisible перенесены в голову (после TitleLocation) — SSL пины,
    // см. слот выше; coverage-витнессы (Таблица_Видимость/Доступность — по одному свойству
    // на форму) новой позиции не противоречат (всё ДО DataPath).
    DesSlot::F(tb::F_DATA_PATH),
    // RefreshRequest — СРАЗУ после DataPath, ДО RowFilter (ERP-witness ВыполнениеОпераций2_2:
    // DataPath→RefreshRequest→RowFilter).
    DesSlot::F(F_TB_REFRESH_REQUEST),
    DesSlot::F(tb::F_ROW_PICTURE_DATA_PATH),
    DesSlot::F(tb::F_ROWS_PICTURE),
    DesSlot::F(tb::F_TEXT_COLOR),
    DesSlot::F(tb::F_BACK_COLOR),
    DesSlot::F(tb::F_BORDER_COLOR),
    DesSlot::F(tb::F_TITLE),
    // TitleTextColor — СРАЗУ после Title, ДО CommandSet (witness СопоставлениеОбъектов…:
    // Title→TitleTextColor→CommandSet).
    DesSlot::F(tb::F_TITLE_TEXT_COLOR),
    // TitleFont/Font — композит-шрифты Таблицы (ERP; точные Designer-пары не
    // витнессированы — за Title-блоком, loose).
    DesSlot::TitleFont,
    DesSlot::Font,
    // TitleHeight — в Title-блоке (ERP n=7, соседи не витнессированы — loose);
    // BehaviorOnHorizontalCompression — после Title, ДО DL-блока (ERP-witness
    // СообщенияФССОбИзмененииСостоянийЭЛН: Title→BehaviorOnHorizontalCompression→AutoRefresh).
    DesSlot::F(F_TB_TITLE_HEIGHT),
    DesSlot::F(F_TB_BEHAVIOR_ON_HORIZONTAL_COMPRESSION),
    DesSlot::TableCommandSet,
    // toolTip/toolTipRepresentation — ПОСЛЕ CommandSet (корпус Designer: CommandSet,
    // ExcludedCommand, ToolTip, ToolTipRepresentation, SearchStringLocation).
    DesSlot::F(tb::F_TOOL_TIP),
    DesSlot::F(tb::F_TOOL_TIP_REPRESENTATION),
    // Shortcut — после ToolTip-блока (ERP n=3, соседи не витнессированы — как у полей; loose).
    DesSlot::F(F_TB_SHORTCUT),
    DesSlot::F(tb::F_SEARCH_STRING_LOCATION),
    DesSlot::F(tb::F_VIEW_STATUS_LOCATION),
    DesSlot::F(tb::F_SEARCH_CONTROL_LOCATION),
    // groupHorizontalAlign/groupVerticalAlign/currentRowUse — ПОСЛЕ SearchControlLocation, ДО
    // ShowCommandBar (корпус Designer: SearchControlLocation→CurrentRowUse→ShowCommandBar);
    // cellHyperlinksRepresentation — ПОСЛЕ ShowCommandBar, ДО DynamicList (prev=ShowCommandBar,
    // next=AutoRefresh).
    DesSlot::F(tb::F_GROUP_HORIZONTAL_ALIGN),
    DesSlot::F(tb::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(tb::F_CURRENT_ROW_USE),
    // viewMode/settingsNamedItemDetailedRepresentation — ПОСЛЕ CurrentRowUse, ДО ShowCommandBar/
    // ContextMenu (SSL Designer: CurrentRowUse<ViewMode, CommandSet<ViewMode×6, ViewMode<
    // SettingsNamedItemDetailedRepresentation×11). Прежде шли сразу после Title (перед CommandSet) —
    // ломало CommandSet→…→ViewMode (напр. НастройкиСинхронизацииФайлов.ФормаЗаписи).
    DesSlot::F(tb::F_VIEW_MODE),
    DesSlot::F(tb::F_COMPLEX_SETTINGS_VIEW_MODE),
    DesSlot::F(tb::F_SETTINGS_NAMED_ITEM_DETAILED_REPRESENTATION),
    DesSlot::TableShowCommandBar,
    DesSlot::F(tb::F_CELL_HYPERLINKS_REPRESENTATION),
    // autoMaxCardHeight — ПЕРЕД DynamicList-блоком (SSL-витнесс: AutoMaxCardHeight→AutoRefresh×4,
    // ShowCommandBar→AutoMaxCardHeight×2, AutoMaxCardHeight→RowFilter×1). Прежняя позиция (после
    // DynamicList) роняла порядок в списковых формах (напр. Подписанты/Настройки.ФормаСписка).
    DesSlot::F(tb::F_AUTO_MAX_CARD_HEIGHT),
    DesSlot::TableDynamicList,
    DesSlot::F(tb::F_ROW_FILTER),
    DesSlot::ContextMenu,
    DesSlot::TableAutoCommandBar,
    DesSlot::ExtendedTooltip,
    DesSlot::TableAdditions,
    DesSlot::Events,
    DesSlot::ChildItems,
];

// ==================== Table: DynamicListTableExtInfo (LANE-F-10) ====================

/// Проекция полей `form:DynamicListTableExtInfo` (EDT-обёртка `<extInfo>` ⟺ Designer-инлайн
/// `<Table>`). Порядок массива = КАНОНИЧЕСКИЙ EDT-порядок эмиссии (метамодель
/// DynamicListTableExtInfo) — оба ридера пушат bag в этом порядке ⇒ X-сравним; EDT-writer
/// эмитит в нём же. Порядок сверен по 230 EDT-экземплярам SSL (напр.
/// `autoRefresh autoRefreshPeriod period restoreCurrentRow topLevelParent showRoot allowRootChoice
/// allowGettingCurrentRowURL userSettingsGroup`, `period choiceFoldersAndItems topLevelParent`).
/// Отличается от [`DES_DYNAMIC_LIST_ORDER`] лишь перестановкой хвоста
/// `allowGettingCurrentRowURL`↔`userSettingsGroup`. Пер-форматные политики — cross-omission-
/// свидетели:
/// * `autoRefreshPeriod`/`period`/`topLevelParent`/`userSettingsGroup` — Symmetric (оба эмитят).
/// * `showRoot`/`allowGettingCurrentRowURL`/`allowRootChoice` — Keep: EDT эмитит лишь `true`
///   (омит `false`), Designer эмитит ВСЕГДА (`DesOmit::Never`).
/// * `autoRefresh`/`choiceFoldersAndItems`/`restoreCurrentRow`/`updateOnDataChange` — Keep: EDT
///   эмитит лишь НЕ-дефолт (`edt_omit`), Designer эмитит ВСЕГДА (`DesOmit::Never`). ПРЕЖДЕ эти
///   поля стояли в КОНЦЕ массива по неверному допущению «EDT их не эмитит никогда» — но SSL EDT
///   несёт non-default (autoRefresh=true / choiceFoldersAndItems=Folders / restoreCurrentRow=true)
///   в их МЕТАМОДЕЛЬНОЙ позиции; порядок исправлен.
pub(crate) static DYNAMIC_LIST_EXT: &[FieldProj] = &[
    fp(
        tb::F_DL_AUTO_REFRESH,
        "autoRefresh",
        "AutoRefresh",
        Region::Ext,
        Codec::Bool,
        keep(
            tb::DL_AUTO_REFRESH_DEFAULT,
            Some(tb::DL_AUTO_REFRESH_DEFAULT),
            tb::DL_AUTO_REFRESH_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_AUTO_REFRESH_PERIOD,
        "autoRefreshPeriod",
        "AutoRefreshPeriod",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        tb::F_DL_PERIOD,
        "period",
        "Period",
        Region::Ext,
        Codec::Period,
        Policy::Symmetric,
    ),
    fp(
        tb::F_DL_CHOICE_FOLDERS_AND_ITEMS,
        "choiceFoldersAndItems",
        "ChoiceFoldersAndItems",
        Region::Ext,
        Codec::EnumTok,
        keep(
            tb::DL_CHOICE_FOLDERS_AND_ITEMS_DEFAULT,
            Some(tb::DL_CHOICE_FOLDERS_AND_ITEMS_DEFAULT),
            tb::DL_CHOICE_FOLDERS_AND_ITEMS_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_RESTORE_CURRENT_ROW,
        "restoreCurrentRow",
        "RestoreCurrentRow",
        Region::Ext,
        Codec::Bool,
        keep(
            tb::DL_RESTORE_CURRENT_ROW_DEFAULT,
            Some(tb::DL_RESTORE_CURRENT_ROW_DEFAULT),
            tb::DL_RESTORE_CURRENT_ROW_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_TOP_LEVEL_PARENT,
        "topLevelParent",
        "TopLevelParent",
        Region::Ext,
        Codec::UndefinedValue,
        Policy::Symmetric,
    ),
    fp(
        tb::F_DL_SHOW_ROOT,
        "showRoot",
        "ShowRoot",
        Region::Ext,
        Codec::Bool,
        keep(
            tb::DL_BOOL_FALSE_DEFAULT,
            Some(tb::DL_BOOL_FALSE_DEFAULT),
            tb::DL_BOOL_FALSE_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_ALLOW_ROOT_CHOICE,
        "allowRootChoice",
        "AllowRootChoice",
        Region::Ext,
        Codec::Bool,
        keep(
            tb::DL_BOOL_FALSE_DEFAULT,
            Some(tb::DL_BOOL_FALSE_DEFAULT),
            tb::DL_BOOL_FALSE_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_UPDATE_ON_DATA_CHANGE,
        "updateOnDataChange",
        "UpdateOnDataChange",
        Region::Ext,
        Codec::EnumTok,
        keep(
            tb::DL_UPDATE_ON_DATA_CHANGE_DEFAULT,
            Some(tb::DL_UPDATE_ON_DATA_CHANGE_DEFAULT),
            tb::DL_UPDATE_ON_DATA_CHANGE_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_ALLOW_GETTING_CURRENT_ROW_URL,
        "allowGettingCurrentRowURL",
        "AllowGettingCurrentRowURL",
        Region::Ext,
        Codec::Bool,
        keep(
            tb::DL_BOOL_FALSE_DEFAULT,
            Some(tb::DL_BOOL_FALSE_DEFAULT),
            tb::DL_BOOL_FALSE_DEFAULT,
            DesOmit::Never,
        ),
    ),
    fp(
        tb::F_DL_USER_SETTINGS_GROUP,
        "userSettingsGroup",
        "UserSettingsGroup",
        Region::Ext,
        Codec::Text,
        Policy::Symmetric,
    ),
];

/// Designer-порядок эмиссии полей DynamicListTableExtInfo (инлайн в `<Table>` после
/// `<ShowCommandBar>`, ДО `<RowFilter>`/`<ContextMenu>`; topo 8/8 таблиц). Отличается от
/// канонического (EDT) порядка перестановкой `UserSettingsGroup`↔`AllowGettingCurrentRowURL`
/// и позициями Designer-ВСЕГДА-полей.
pub(crate) static DES_DYNAMIC_LIST_ORDER: &[FieldId] = &[
    tb::F_DL_AUTO_REFRESH,
    tb::F_DL_AUTO_REFRESH_PERIOD,
    tb::F_DL_PERIOD,
    tb::F_DL_CHOICE_FOLDERS_AND_ITEMS,
    tb::F_DL_RESTORE_CURRENT_ROW,
    tb::F_DL_TOP_LEVEL_PARENT,
    tb::F_DL_SHOW_ROOT,
    tb::F_DL_ALLOW_ROOT_CHOICE,
    tb::F_DL_UPDATE_ON_DATA_CHANGE,
    tb::F_DL_USER_SETTINGS_GROUP,
    tb::F_DL_ALLOW_GETTING_CURRENT_ROW_URL,
];

