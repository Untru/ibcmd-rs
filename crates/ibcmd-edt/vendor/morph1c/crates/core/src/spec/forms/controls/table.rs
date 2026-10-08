//! Канонический спек контрола `Table` (ARCHITECTURE.md §1.4/§1.6, под-IR L1f, LANE-F-4) —
//! КРУПНЕЙШИЙ композит семейства контролов формы.
//!
//! EDT кодирует Таблицу как `<items xsi:type="form:Table">`; Designer — как `<Table>`.
//! Таблица НЕ несёт extInfo-обёртки: её тип-специфичные поля ИНЛАЙН в теле (сверено по
//! корпусу — единственная extInfo `form:DynamicListTableExtInfo` у динамических списков
//! НЕ моделируется этим срезом → §1.0-ошибка, форма-блокер).
//!
//! # Составные части Таблицы (все — корпусные свидетели)
//! * ~60 полей тела (общие + табличные), ИНЛАЙН, с ПЕР-ФОРМАТНЫМИ дефолтами (cross-omission).
//! * СОБСТВЕННАЯ `<autoCommandBar>` и СОБСТВЕННЫЕ `<excludedCommands>`/`handlers`.
//! * ТРИ добавления (`searchStringAddition`/`viewStatusAddition`/`searchControlAddition`).
//! * КОЛОНКИ (`<items>` — FormField/ColumnGroup, рекурсивно) + decorator-стабы.
//!
//! Каноника (id/порядок/value_kind) здесь; форматная проекция (теги/кодеки/дефолты) —
//! `formats-xml/src/form/tables.rs` (§1.6).

use crate::ir::value::ValueKind;
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::spec::forms::controls::ControlSpec;

// ============================ поля тела (канонический порядок EDT) ============================

// -- HEAD (до excludedCommands/колонок/autoCommandBar) --
pub const F_DISPLAY_IMPORTANCE: FieldId = FieldId(1);
pub const F_TITLE: FieldId = FieldId(2);
pub const F_VISIBLE: FieldId = FieldId(3);
pub const F_ENABLED: FieldId = FieldId(4);
pub const F_USER_VISIBLE: FieldId = FieldId(5);
pub const F_DATA_PATH: FieldId = FieldId(6);
/// Canonical names of the table's current-row source, in both editing languages.
pub const CURRENT_DATA_MEMBER_NAMES: [&str; 2] = ["CurrentData", "ТекущиеДанные"];
/// Table-owned element source path. The table and source identities must then
/// be matched by the adapter's existing identifier comparison against CURRENT items.
pub fn table_element_member_path(path: &str) -> Option<(&str, &str, &str)> {
    let (items, tail) = path.split_once('.')?;
    if !matches!(items, "Items" | "Элементы") {
        return None;
    }
    let (table, tail) = tail.split_once('.')?;
    let (current, member) = tail.split_once('.')?;
    if table.is_empty() || current.is_empty() || member.is_empty() {
        return None;
    }
    Some((table, current, member))
}
pub const F_DEFAULT_ITEM: FieldId = FieldId(7);
pub const F_SKIP_ON_INPUT: FieldId = FieldId(8);
pub const F_TITLE_LOCATION: FieldId = FieldId(9);
// -- HEAD_B (после колонок, до autoCommandBar) --
pub const F_COMMAND_BAR_LOCATION: FieldId = FieldId(10);
pub const F_TOOL_TIP_REPRESENTATION: FieldId = FieldId(11);
// -- TAIL (после decorator-стабов) --
pub const F_REPRESENTATION: FieldId = FieldId(12);
pub const F_AUTO_FILL: FieldId = FieldId(13);
pub const F_READ_ONLY: FieldId = FieldId(14);
pub const F_CHANGE_ROW_SET: FieldId = FieldId(15);
pub const F_CHANGE_ROW_ORDER: FieldId = FieldId(16);
pub const F_WIDTH: FieldId = FieldId(17);
pub const F_AUTO_MAX_WIDTH: FieldId = FieldId(18);
pub const F_HEIGHT: FieldId = FieldId(19);
pub const F_AUTO_MAX_HEIGHT: FieldId = FieldId(20);
/// `maxWidth` — макс. ширина таблицы (Int, симметрично; метамодель после `autoMaxWidth`,
/// до `height`). Witness — Table ПоискИУдалениеДублей.ПравилаПоиска maxWidth=60.
pub const F_MAX_WIDTH: FieldId = FieldId(211);
pub const F_HEIGHT_IN_TABLE_ROWS: FieldId = FieldId(21);
pub const F_HEIGHT_CONTROL_VARIANT: FieldId = FieldId(22);
pub const F_AUTO_MAX_ROWS_COUNT: FieldId = FieldId(23);
pub const F_CHOICE_MODE: FieldId = FieldId(24);
pub const F_ROW_INPUT_MODE: FieldId = FieldId(25);
pub const F_SELECTION_MODE: FieldId = FieldId(26);
pub const F_ROW_SELECTION_MODE: FieldId = FieldId(27);
pub const F_ROW_ACTIONS_SHOW_TYPE: FieldId = FieldId(28);
pub const F_HEADER: FieldId = FieldId(29);
pub const F_MAX_ROWS_COUNT: FieldId = FieldId(30);
pub const F_HEADER_HEIGHT: FieldId = FieldId(31);
pub const F_FOOTER_HEIGHT: FieldId = FieldId(32);
pub const F_HORIZONTAL_SCROLL_BAR: FieldId = FieldId(33);
pub const F_VERTICAL_SCROLL_BAR: FieldId = FieldId(34);
pub const F_HORIZONTAL_LINES: FieldId = FieldId(35);
pub const F_VERTICAL_LINES: FieldId = FieldId(36);
pub const F_HORIZONTAL_LINES_BWA: FieldId = FieldId(37);
pub const F_VERTICAL_LINES_BWA: FieldId = FieldId(38);
pub const F_USE_ALTERNATION_ROW_COLOR_BWA: FieldId = FieldId(39);
pub const F_AUTO_INSERT_NEW_ROW: FieldId = FieldId(40);
pub const F_AUTO_ADD_INCOMPLETE: FieldId = FieldId(41);
pub const F_AUTO_MARK_INCOMPLETE: FieldId = FieldId(42);
pub const F_SEARCH_ON_INPUT: FieldId = FieldId(43);
pub const F_INITIAL_LIST_VIEW: FieldId = FieldId(44);
pub const F_INITIAL_TREE_VIEW: FieldId = FieldId(45);
pub const F_INITIAL_ROW_ACTIVATION: FieldId = FieldId(46);
pub const F_HORIZONTAL_STRETCH: FieldId = FieldId(47);
pub const F_VERTICAL_STRETCH: FieldId = FieldId(48);
pub const F_ENABLE_START_DRAG: FieldId = FieldId(49);
pub const F_ENABLE_DRAG: FieldId = FieldId(50);
pub const F_FILE_DRAG_MODE: FieldId = FieldId(51);
pub const F_ROW_PICTURE_DATA_PATH: FieldId = FieldId(52);
pub const F_ROWS_PICTURE: FieldId = FieldId(53);
pub const F_SEARCH_STRING_LOCATION: FieldId = FieldId(54);
pub const F_VIEW_STATUS_LOCATION: FieldId = FieldId(55);
pub const F_SEARCH_CONTROL_LOCATION: FieldId = FieldId(56);
pub const F_AUTO_MAX_CARD_HEIGHT: FieldId = FieldId(57);
pub const F_ROW_FILTER: FieldId = FieldId(58);
pub const F_VIEW_MODE: FieldId = FieldId(59);
pub const F_SETTINGS_NAMED_ITEM_DETAILED_REPRESENTATION: FieldId = FieldId(60);
/// `multipleChoice` — множественный выбор (Bool, симметрично: оба эмитят `true`, оба опускают
/// дефолт false; сверено корпусом покрытия). EDT-позиция — после `autoMaxRowsCount`.
pub const F_MULTIPLE_CHOICE: FieldId = FieldId(61);
/// `output` — вывод (Enum, симметрично: оба эмитят `Enable`, оба опускают дефолт `Use`;
/// сверено корпусом покрытия). EDT-позиция — после `initialRowActivation`.
pub const F_OUTPUT: FieldId = FieldId(62);
/// `toolTip` — подсказка (Localized, симметрично; метамодель до `toolTipRepresentation`).
pub const F_TOOL_TIP: FieldId = FieldId(63);
/// `maxHeight` — макс. высота (Int, симметрично; метамодель после `autoMaxHeight`).
pub const F_MAX_HEIGHT: FieldId = FieldId(64);
/// `footer` — подвал (Bool, симметрично; метамодель после `headerHeight`, до `footerHeight`).
pub const F_FOOTER: FieldId = FieldId(65);
/// `textColor` — цвет текста (Color-Ref, симметрично; метамодель после `rowsPicture`).
pub const F_TEXT_COLOR: FieldId = FieldId(66);
/// `backColor` — цвет фона (Color-Ref, симметрично; метамодель после `textColor`).
pub const F_BACK_COLOR: FieldId = FieldId(67);
/// `borderColor` — цвет рамки (Color-Ref, симметрично; метамодель после `backColor`).
pub const F_BORDER_COLOR: FieldId = FieldId(68);
/// `cellHyperlinksRepresentation` — представление гиперссылок ячеек (Enum, симметрично;
/// метамодель после `shapeSaveColorsKey`, до `searchStringLocation`).
pub const F_CELL_HYPERLINKS_REPRESENTATION: FieldId = FieldId(69);
/// `groupHorizontalAlign` — гориз. выравнивание в группе (Enum, симметрично; метамодель после
/// `verticalScrollPanelVisible`, до `currentRowUse`).
pub const F_GROUP_HORIZONTAL_ALIGN: FieldId = FieldId(70);
/// `groupVerticalAlign` — верт. выравнивание в группе (Enum, симметрично; после `groupHorizontalAlign`).
pub const F_GROUP_VERTICAL_ALIGN: FieldId = FieldId(71);
/// `currentRowUse` — использование текущей строки (Enum, симметрично; метамодель после
/// `refreshRequest`, до `autoMaxCardHeight`).
pub const F_CURRENT_ROW_USE: FieldId = FieldId(72);
/// `complexSettingsViewMode` — режим показа сложных настроек Таблицы (Enum, симметрично;
/// метамодель после `viewMode`, витнесс `Show`).
pub const F_COMPLEX_SETTINGS_VIEW_MODE: FieldId = FieldId(73);
/// `titleTextColor` — цвет текста заголовка (Color-Ref, симметрично; метамодель СРАЗУ после
/// `title`, до `titleFont`/`visible`). Witness — Table СопоставлениеОбъектовИнформационныхБаз.
/// Форма «ТаблицаСопоставления»: EDT `title→titleTextColor→visible`, Designer
/// `Title→TitleTextColor→CommandSet` (Style.ПоясняющийТекст ⟷ style:ПоясняющийТекст).
pub const F_TITLE_TEXT_COLOR: FieldId = FieldId(212);

// -- поля DynamicListTableExtInfo (`form:DynamicListTableExtInfo`, LANE-F-10) --
// Отдельное id-пространство (200+): эти поля живут в СОБСТВЕННОМ bag'е
// [`crate::ir::DynamicListExt::fields`], не в теле Таблицы. EDT-обёртка `<extInfo>` ⟺
// Designer-инлайн `<Table>`. Порядок id = канонический (= EDT-эмиссия).
/// `autoRefresh` (Designer `AutoRefresh`) — EDT НЕ эмитит НИКОГДА; Designer эмитит ВСЕГДА
/// (константа корпуса `false`). Keep-поле.
pub const F_DL_AUTO_REFRESH: FieldId = FieldId(200);
/// `autoRefreshPeriod` (Designer `AutoRefreshPeriod`) — период автообновления (сек). Оба эмитят.
pub const F_DL_AUTO_REFRESH_PERIOD: FieldId = FieldId(201);
/// `period` (Designer `Period`) — стандартный период `{startDate,endDate}`. EDT — 2 листа;
/// Designer — `<v8:variant xsi:type="v8:StandardPeriodVariant">Custom` + `v8:startDate`/
/// `v8:endDate`. Канон — `List([Str(start), Str(end)])`; `variant`=`Custom` — Designer-каркас.
pub const F_DL_PERIOD: FieldId = FieldId(202);
/// `choiceFoldersAndItems` (Designer `ChoiceFoldersAndItems`) — EDT НЕ эмитит; Designer ВСЕГДА
/// (константа `Items`). Keep-поле.
pub const F_DL_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(203);
/// `restoreCurrentRow` (Designer `RestoreCurrentRow`) — EDT НЕ эмитит; Designer ВСЕГДА
/// (константа `false`). Keep-поле.
pub const F_DL_RESTORE_CURRENT_ROW: FieldId = FieldId(204);
/// `topLevelParent` (Designer `TopLevelParent`) — EDT `xsi:type="core:UndefinedValue"` ⟺
/// Designer `xsi:nil="true"` (самозакрытые). Канон — `Bool(true)` (присутствие). Оба эмитят.
pub const F_DL_TOP_LEVEL_PARENT: FieldId = FieldId(205);
/// `showRoot` (Designer `ShowRoot`) — EDT эмитит лишь `true` (омит `false`, EDT-дефолт `false`);
/// Designer эмитит ВСЕГДА. Keep-поле.
pub const F_DL_SHOW_ROOT: FieldId = FieldId(206);
/// `allowRootChoice` (Designer `AllowRootChoice`) — EDT НЕ эмитит; Designer ВСЕГДА
/// (константа `false`). Keep-поле.
pub const F_DL_ALLOW_ROOT_CHOICE: FieldId = FieldId(207);
/// `updateOnDataChange` (Designer `UpdateOnDataChange`) — EDT НЕ эмитит; Designer ВСЕГДА
/// (константа `Auto`). Keep-поле.
pub const F_DL_UPDATE_ON_DATA_CHANGE: FieldId = FieldId(208);
/// `userSettingsGroup` (Designer `UserSettingsGroup`) — имя группы польз. настроек, если задано.
/// Оба формата эмитят при наличии. Symmetric.
pub const F_DL_USER_SETTINGS_GROUP: FieldId = FieldId(209);
/// `allowGettingCurrentRowURL` (Designer `AllowGettingCurrentRowURL`) — EDT эмитит лишь `true`
/// (омит `false`); Designer эмитит ВСЕГДА. Keep-поле.
pub const F_DL_ALLOW_GETTING_CURRENT_ROW_URL: FieldId = FieldId(210);

/// Пер-форматные дефолты полей DynamicListTableExtInfo (cross-omission-свидетели корпуса, 8/8
/// таблиц): значения Designer-ВСЕГДА-эмитимых полей, которые EDT опускает.
pub const DL_AUTO_REFRESH_DEFAULT: &str = "false";
pub const DL_CHOICE_FOLDERS_AND_ITEMS_DEFAULT: &str = "Items";
pub const DL_RESTORE_CURRENT_ROW_DEFAULT: &str = "false";
pub const DL_BOOL_FALSE_DEFAULT: &str = "false";
pub const DL_UPDATE_ON_DATA_CHANGE_DEFAULT: &str = "Auto";

// -- поля ДОБАВЛЕНИЯ (searchString/viewStatus/searchControl) --
/// `source` добавления (EDT `<source>` ⟺ Designer `<AdditionSource><Item>`) — dataPath-ссылка.
pub const F_ADDITION_SOURCE: FieldId = FieldId(101);
/// `autoMaxWidth` добавления (extInfo) — OppositeBool-семантика: канон-bag несёт ТОЛЬКО
/// `Bool(true)` (sparse против false). EDT эмитит `true`/опускает `false` (1847× true,
/// 2× absent); Designer эмитит `<AutoMaxWidth>false`/опускает `true` (2× false —
/// ЗагрузкаКурсовВалют, РасширенныйВводКонтактнойИнформации).
pub const F_ADDITION_AUTO_MAX_WIDTH: FieldId = FieldId(102);
/// `title` добавления (`<title>` EDT ⟺ `<Title>` Designer) — локализованный заголовок,
/// если задан (у SearchString-добавления-контрола в панели). X-сравним.
pub const F_ADDITION_TITLE: FieldId = FieldId(103);
/// `visible` добавления Таблицы (EDT `<visible>` ⟺ Designer `<Visible>`) — plain-bool, SPARSE:
/// оба формата ОПУСКАЮТ дефолт `true`, эмитят лишь `false`. X-сравним (хранится когда присутствует).
pub const F_ADDITION_VISIBLE: FieldId = FieldId(104);
/// `enabled` добавления Таблицы (EDT `<enabled>` ⟺ Designer `<Enabled>`) — plain-bool, SPARSE:
/// оба формата ОПУСКАЮТ дефолт `true`, эмитят лишь `false` (witnessed: АвтономнаяРаботаВМоделиСервиса/
/// НастройкиСинхронизацииДанных). X-сравним (хранится когда присутствует; расхождение ⇒ X=FALSE).
pub const F_ADDITION_ENABLED: FieldId = FieldId(105);
/// `groupHorizontalAlign` добавления-контрола (ТЕЛО: EDT после `source`, метамодель Addition
/// type→source→**groupHorizontalAlign**; Designer после `Title`) — Enum, Symmetric (5⟷5 `Right`).
pub const F_ADDITION_GROUP_HORIZONTAL_ALIGN: FieldId = FieldId(106);
/// `width` добавления (extInfo, метамодель SearchStringAdditionExtInfo width→autoMaxWidth) —
/// Int, Symmetric (witness ГрупповоеИзменениеРеквизитов.ВыбранныеЭлементы 40⟷40; Designer
/// инлайн `<Width>` между AdditionSource и HorizontalStretch).
pub const F_ADDITION_WIDTH: FieldId = FieldId(107);
/// `horizontalStretch` добавления (extInfo, метамодель после autoMaxWidth/minWidth) — Bool,
/// Symmetric (EDT false×3/true×2 ⟷ Designer false×3/true×2).
pub const F_ADDITION_HORIZONTAL_STRETCH: FieldId = FieldId(108);

// ============================ пер-форматные keep-дефолты ============================

/// `titleLocation`: EDT опускает `Auto`; Designer опускает `None`.
pub const TITLE_LOCATION_EDT_DEFAULT: &str = "Auto";
pub const TITLE_LOCATION_DESIGNER_DEFAULT: &str = "None";
/// `representation`: EDT опускает `List`; Designer опускает `HierarchicalList`.
pub const REPRESENTATION_EDT_DEFAULT: &str = "List";
pub const REPRESENTATION_DESIGNER_DEFAULT: &str = "HierarchicalList";
/// `selectionMode`: EDT опускает `SingleRow`; Designer опускает `MultiRow`.
pub const SELECTION_MODE_EDT_DEFAULT: &str = "SingleRow";
pub const SELECTION_MODE_DESIGNER_DEFAULT: &str = "MultiRow";
/// `rowSelectionMode`: EDT опускает `Cell`; Designer опускает `Auto`.
pub const ROW_SELECTION_MODE_EDT_DEFAULT: &str = "Cell";
pub const ROW_SELECTION_MODE_DESIGNER_DEFAULT: &str = "Auto";
/// `horizontalScrollBar`: EDT опускает `DontUse`; Designer опускает `AutoUse`.
pub const HORIZONTAL_SCROLL_BAR_EDT_DEFAULT: &str = "DontUse";
/// `verticalScrollBar`/`horizontalScrollBar` Designer-дефолт (омиссия `AutoUse`).
pub const SCROLL_BAR_DESIGNER_DEFAULT: &str = "AutoUse";
/// `fileDragMode`: EDT опускает `AsFile`; Designer опускает `AsFileRef`.
pub const FILE_DRAG_MODE_EDT_DEFAULT: &str = "AsFile";
pub const FILE_DRAG_MODE_DESIGNER_DEFAULT: &str = "AsFileRef";
/// `searchOnInput`/`initialListView` Designer-дефолт (омиссия `Auto`; EDT эмитит всегда).
pub const AUTO_DEFAULT: &str = "Auto";
/// EDT-дефолт `searchOnInput` (EDT омитит `Use`; Designer омитит `Auto`; сверено корпусом
/// покрытия — Designer эмитит `Use`/`DontUse`, EDT эмитит `Auto`/`DontUse`).
pub const SEARCH_ON_INPUT_EDT_DEFAULT: &str = "Use";
/// EDT-дефолт `initialListView` (EDT омитит `Beginning`; Designer омитит `Auto`; сверено
/// корпусом — Designer эмитит `Beginning`/`End`, EDT эмитит `Auto`/`End`).
pub const INITIAL_LIST_VIEW_EDT_DEFAULT: &str = "Beginning";
/// `headerHeight`: EDT эмитит ВСЕГДА `1`; Designer НЕ несёт (fill `1`).
pub const HEADER_HEIGHT_FILL: &str = "1";
/// `footerHeight`: EDT опускает `0`; Designer опускает `1`.
pub const FOOTER_HEIGHT_EDT_DEFAULT: &str = "0";
pub const FOOTER_HEIGHT_DESIGNER_DEFAULT: &str = "1";

// ============================ построение спека ============================

fn en(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::required(id, name, ValueKind::Enum)
}
fn bl(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::required(id, name, ValueKind::Bool)
}
fn it(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::required(id, name, ValueKind::Int)
}
fn rf(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::required(id, name, ValueKind::Ref)
}
fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(
        id,
        name,
        ValueKind::Localized,
        crate::ir::value::PropertyValue::Localized(Vec::new()),
    )
    .normalized(Normalize::LocalizedSortByLang)
}

/// Тело Таблицы в КАНОНИЧЕСКОМ порядке (= EDT-порядок эмиссии, topo по корпусу).
fn build_body_fields() -> Vec<FieldSpec> {
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
        en(F_COMMAND_BAR_LOCATION, "commandBarLocation"),
        en(F_TOOL_TIP_REPRESENTATION, "toolTipRepresentation"),
        en(F_REPRESENTATION, "representation"),
        bl(F_AUTO_FILL, "autoFill"),
        bl(F_READ_ONLY, "readOnly"),
        bl(F_CHANGE_ROW_SET, "changeRowSet"),
        bl(F_CHANGE_ROW_ORDER, "changeRowOrder"),
        it(F_WIDTH, "width"),
        bl(F_AUTO_MAX_WIDTH, "autoMaxWidth"),
        it(F_MAX_WIDTH, "maxWidth"),
        it(F_HEIGHT, "height"),
        bl(F_AUTO_MAX_HEIGHT, "autoMaxHeight"),
        it(F_HEIGHT_IN_TABLE_ROWS, "heightInTableRows"),
        en(F_HEIGHT_CONTROL_VARIANT, "heightControlVariant"),
        bl(F_AUTO_MAX_ROWS_COUNT, "autoMaxRowsCount"),
        // maxRowsCount — МЕТАМОДЕЛЬ Table: autoMaxRowsCount → maxRowsCount → choiceMode
        // (SSL: heightControlVariant<maxRowsCount×13, maxRowsCount<selectionMode×6/header×4;
        // прежняя позиция после header — вскрытый МашиночитаемыеДоверенности write-баг).
        it(F_MAX_ROWS_COUNT, "maxRowsCount"),
        bl(F_MULTIPLE_CHOICE, "multipleChoice"),
        bl(F_CHOICE_MODE, "choiceMode"),
        en(F_ROW_INPUT_MODE, "rowInputMode"),
        en(F_SELECTION_MODE, "selectionMode"),
        en(F_ROW_SELECTION_MODE, "rowSelectionMode"),
        en(F_ROW_ACTIONS_SHOW_TYPE, "rowActionsShowType"),
        bl(F_HEADER, "header"),
        it(F_HEADER_HEIGHT, "headerHeight"),
        it(F_FOOTER_HEIGHT, "footerHeight"),
        en(F_HORIZONTAL_SCROLL_BAR, "horizontalScrollBar"),
        en(F_VERTICAL_SCROLL_BAR, "verticalScrollBar"),
        bl(F_HORIZONTAL_LINES, "horizontalLines"),
        bl(F_VERTICAL_LINES, "verticalLines"),
        bl(F_HORIZONTAL_LINES_BWA, "horizontalLinesBWA"),
        bl(F_VERTICAL_LINES_BWA, "verticalLinesBWA"),
        bl(F_USE_ALTERNATION_ROW_COLOR_BWA, "useAlternationRowColorBWA"),
        bl(F_AUTO_INSERT_NEW_ROW, "autoInsertNewRow"),
        bl(F_AUTO_ADD_INCOMPLETE, "autoAddIncomplete"),
        bl(F_AUTO_MARK_INCOMPLETE, "autoMarkIncomplete"),
        en(F_SEARCH_ON_INPUT, "searchOnInput"),
        en(F_INITIAL_LIST_VIEW, "initialListView"),
        en(F_INITIAL_TREE_VIEW, "initialTreeView"),
        en(F_INITIAL_ROW_ACTIVATION, "initialRowActivation"),
        en(F_OUTPUT, "output"),
        bl(F_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_VERTICAL_STRETCH, "verticalStretch"),
        bl(F_ENABLE_START_DRAG, "enableStartDrag"),
        bl(F_ENABLE_DRAG, "enableDrag"),
        en(F_FILE_DRAG_MODE, "fileDragMode"),
        rf(F_ROW_PICTURE_DATA_PATH, "rowPictureDataPath"),
        rf(F_ROWS_PICTURE, "rowsPicture"),
        en(F_SEARCH_STRING_LOCATION, "searchStringLocation"),
        en(F_VIEW_STATUS_LOCATION, "viewStatusLocation"),
        en(F_SEARCH_CONTROL_LOCATION, "searchControlLocation"),
        bl(F_AUTO_MAX_CARD_HEIGHT, "autoMaxCardHeight"),
        bl(F_ROW_FILTER, "rowFilter"),
        en(F_VIEW_MODE, "viewMode"),
        en(F_COMPLEX_SETTINGS_VIEW_MODE, "complexSettingsViewMode"),
        bl(
            F_SETTINGS_NAMED_ITEM_DETAILED_REPRESENTATION,
            "settingsNamedItemDetailedRepresentation",
        ),
        loc(F_TOOL_TIP, "toolTip"),
        it(F_MAX_HEIGHT, "maxHeight"),
        bl(F_FOOTER, "footer"),
        rf(F_TEXT_COLOR, "textColor"),
        rf(F_BACK_COLOR, "backColor"),
        rf(F_BORDER_COLOR, "borderColor"),
        en(
            F_CELL_HYPERLINKS_REPRESENTATION,
            "cellHyperlinksRepresentation",
        ),
        en(F_GROUP_HORIZONTAL_ALIGN, "groupHorizontalAlign"),
        en(F_GROUP_VERTICAL_ALIGN, "groupVerticalAlign"),
        en(F_CURRENT_ROW_USE, "currentRowUse"),
    ]
}

fn leak_spec(entity: &'static str, fields: Vec<FieldSpec>) -> EntitySpec {
    EntitySpec {
        entity,
        fields: Box::leak(fields.into_boxed_slice()),
        children: &[],
    }
}

/// Канонический спек тела Таблицы (кэш на процесс).
pub fn table_body() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("Table", build_body_fields()))
}

fn empty_ext() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("TableExtInfo", Vec::new()))
}

/// Канонический [`ControlSpec`] вида `Table` (контейнер: несёт колонки).
pub fn table() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "Table",
        properties: table_body(),
        ext_info: empty_ext(),
        container: true,
    })
}
