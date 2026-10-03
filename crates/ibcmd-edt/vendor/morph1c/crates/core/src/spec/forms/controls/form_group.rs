//! Канонический спек семейства контейнер-контролов `FormGroup` — ARCHITECTURE.md
//! §1.4/§1.6, под-IR L1f. Четыре вида среза LANE-F-2: `UsualGroup`, `Pages`, `Page`,
//! `ButtonGroup` (все — контейнеры, несут рекурсивных детей).
//!
//! EDT кодирует любой из них как `<items xsi:type="form:FormGroup">` с дискриминатором
//! `<type>UsualGroup|Pages|Page</type>` (у `ButtonGroup` `<type>` ОТСУТСТВУЕТ — это и есть
//! его дискриминатор) и `<extInfo xsi:type="form:<Тип>GroupExtInfo|form:ButtonGroupExtInfo">`;
//! Designer — как элемент по имени вида. ВАЖНО (corpus fact, слом старой модели): дети
//! `<items>` в EDT идут СРАЗУ ПОСЛЕ `<id>` (до свойств), а НЕ после extInfo.
//!
//! # Ключевые корпусные факты (cross-omission witness; UsualGroup 395, Page 110,
//! # ButtonGroup 169, Pages 39)
//! * ОБЩЕЕ тело группы — один спек на семейство; канонический порядок = EDT-порядок
//!   (topo, конфликтов 0).
//! * `representation`/`showTitle`/`behavior`/`group`/`throughAlign` UsualGroup — KEEP-поля с
//!   ПРОТИВОПОЛОЖНЫМИ пер-форматными дефолтами, живущие в EDT-EXTINFO / Designer-inline:
//!   EDT опускает None/false/Usual/Horizontal/Use, Designer опускает Auto/auto (сверено
//!   270+334+307+16+5 cross-omissions). Старая модель (body-поля с дефолтами
//!   None/WeakSeparation) корпусом ОПРОВЕРГНУТА и заменена.
//! * `slaveItemsWidth` (EDT) ⟺ `ChildItemsWidth` (Designer) — РАЗНЫЕ имена тегов, одно поле.
//! * Pages `pagesRepresentation`: EDT опускает `None`, Designer опускает `Auto`.
//! * Page `showTitle`: EDT опускает `false`, Designer опускает `auto`; `scrollOnCompress`
//!   СИММЕТРИЧЕН (оба эмитят явные true/false — 102/110).
//! * События групп живут в EDT-EXTINFO `<handlers>` (Pages `OnCurrentPageChange`) /
//!   Designer `<Events>`.

use crate::ir::value::ValueKind;
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::spec::forms::controls::ControlSpec;

// ============================ общее тело группы ============================

/// `displayImportance` — важность отображения (EDT элемент / Designer атрибут).
pub const F_DISPLAY_IMPORTANCE: FieldId = FieldId(1);
/// `visible` — видимость (противоположные bool-дефолты).
pub const F_VISIBLE: FieldId = FieldId(2);
/// `enabled` — доступность (EDT эмитит всегда; Designer опускает true).
pub const F_ENABLED: FieldId = FieldId(3);
/// `userVisible` — пользовательская видимость (CommonBool-кодировка).
pub const F_USER_VISIBLE: FieldId = FieldId(4);
/// `title` — локализованный заголовок (оба формата).
pub const F_TITLE: FieldId = FieldId(5);
/// `readOnly` — только просмотр (Bool, симметрично).
pub const F_READ_ONLY: FieldId = FieldId(6);
/// `toolTip` — локализованная подсказка (оба формата).
pub const F_TOOL_TIP: FieldId = FieldId(7);
/// `enableContentChange` — разрешить изменение состава (Bool, симметрично).
pub const F_ENABLE_CONTENT_CHANGE: FieldId = FieldId(8);
/// `shortcut` — горячая клавиша (Str, симметрично).
pub const F_SHORTCUT: FieldId = FieldId(9);
/// `toolTipRepresentation` — представление подсказки (Enum, симметрично).
pub const F_TOOL_TIP_REPRESENTATION: FieldId = FieldId(10);
/// `width` — ширина (Int, симметрично).
pub const F_WIDTH: FieldId = FieldId(11);
/// `horizontalStretch` — растяжение по горизонтали (Bool, симметрично: явные true/false).
pub const F_HORIZONTAL_STRETCH: FieldId = FieldId(12);
/// `verticalStretch` — растяжение по вертикали (Bool, симметрично).
pub const F_VERTICAL_STRETCH: FieldId = FieldId(13);
/// `groupHorizontalAlign` — выравнивание в группе по горизонтали (Enum, симметрично).
pub const F_GROUP_HORIZONTAL_ALIGN: FieldId = FieldId(14);
/// `groupVerticalAlign` — выравнивание в группе по вертикали (Enum, симметрично).
pub const F_GROUP_VERTICAL_ALIGN: FieldId = FieldId(15);
/// `height` — высота группы (Int, симметрично; witness UsualGroup — после `width`).
pub const F_HEIGHT: FieldId = FieldId(16);
/// `titleTextColor` — цвет текста заголовка (Color-Ref, симметрично; witness UsualGroup —
/// после `title`).
pub const F_TITLE_TEXT_COLOR: FieldId = FieldId(17);

// ============================ extInfo: UsualGroup ============================

/// extInfo: `group` — направление группировки (KEEP: EDT опускает `Horizontal`,
/// Designer опускает `Auto`; cross-omission 16/15).
pub const F_EXT_GROUP: FieldId = FieldId(101);
/// extInfo: `horizontalSpacing` (Enum, симметрично).
pub const F_EXT_HORIZONTAL_SPACING: FieldId = FieldId(102);
/// extInfo: `verticalAlign` (Enum, симметрично).
pub const F_EXT_VERTICAL_ALIGN: FieldId = FieldId(103);
/// extInfo: `verticalSpacing` (Enum, симметрично).
pub const F_EXT_VERTICAL_SPACING: FieldId = FieldId(104);
/// extInfo: `horizontalAlign` (Enum, симметрично).
pub const F_EXT_HORIZONTAL_ALIGN: FieldId = FieldId(105);
/// extInfo: `behavior` (KEEP: EDT опускает `Usual`, Designer опускает `Auto`; 307 witness).
pub const F_EXT_BEHAVIOR: FieldId = FieldId(106);
/// extInfo: `collapsedRepresentationTitle` (Localized, симметрично).
pub const F_EXT_COLLAPSED_REPRESENTATION_TITLE: FieldId = FieldId(107);
/// extInfo: `collapsed` (Bool, симметрично).
pub const F_EXT_COLLAPSED: FieldId = FieldId(108);
/// extInfo: `controlRepresentation` (Enum, симметрично).
pub const F_EXT_CONTROL_REPRESENTATION: FieldId = FieldId(109);
/// extInfo: `scrollOnCompress` (Bool, симметрично).
pub const F_EXT_SCROLL_ON_COMPRESS: FieldId = FieldId(110);
/// extInfo: `representation` (KEEP: EDT опускает `None`, Designer опускает `Auto`;
/// 270/48 witness — старый дефолт WeakSeparation ОПРОВЕРГНУТ).
pub const F_EXT_REPRESENTATION: FieldId = FieldId(111);
/// extInfo: `showAsCard` (Bool, симметрично).
pub const F_EXT_SHOW_AS_CARD: FieldId = FieldId(112);
/// extInfo: `showLeftMargin` (противоположные bool-дефолты: EDT true / Designer false).
pub const F_EXT_SHOW_LEFT_MARGIN: FieldId = FieldId(113);
/// extInfo: `united` (противоположные bool-дефолты).
pub const F_EXT_UNITED: FieldId = FieldId(114);
/// extInfo: `backColor` (Color-кодек).
pub const F_EXT_BACK_COLOR: FieldId = FieldId(115);
/// extInfo: `slaveItemsWidth` — EDT-тег `slaveItemsWidth` ⟺ Designer-тег `ChildItemsWidth`.
pub const F_EXT_SLAVE_ITEMS_WIDTH: FieldId = FieldId(116);
/// extInfo: `showTitle` (KEEP: EDT опускает `false`, Designer опускает `auto`; 334/61).
pub const F_EXT_SHOW_TITLE: FieldId = FieldId(117);
/// extInfo: `titleDataPath` (DataPath dual-encoding).
pub const F_EXT_TITLE_DATA_PATH: FieldId = FieldId(118);
/// extInfo: `throughAlign` (KEEP: EDT опускает `Use`, Designer опускает `Auto`; 5/389).
pub const F_EXT_THROUGH_ALIGN: FieldId = FieldId(119);
/// extInfo: `currentRowUse` (KEEP: EDT эмитит ВСЕГДА `Auto`; Designer НИКОГДА — fill `Auto`).
pub const F_EXT_CURRENT_ROW_USE: FieldId = FieldId(120);
/// extInfo: `childrenAlign` — выравнивание подчинённых (UsualGroup/Page). Enum, симметрично:
/// оба формата эмитят не-дефолтное значение, оба опускают дефолт (сверено корпусом покрытия
/// s10_forms). EDT-позиция — сразу после `group`. ⚠ У ШЕСТОГО литерала диалекты расходятся
/// ИМЕНЕМ: EDT `ItemsAutoTitlesLeft` ⟺ Designer `TitlesLeftDataAuto` (ERP ×3) — канон = EDT,
/// перевод в `CHILDREN_ALIGN_MAP` (`formats-xml/form/tables/group.rs`), как у `verticalScroll`
/// и `heightControlVariant`.
pub const F_EXT_CHILDREN_ALIGN: FieldId = FieldId(121);
/// extInfo UsualGroup: `hyperlink` — группа-гиперссылка (Bool, симметрично: оба эмитят `true`,
/// оба опускают дефолт false; сверено корпусом покрытия). EDT-позиция — после `representation`.
pub const F_EXT_HYPERLINK: FieldId = FieldId(122);
/// extInfo Popup: `shape` — форма подменю (Enum, симметрично: `Usual`/`Oval`/…; оба эмитят
/// не-дефолт, оба опускают дефолт). EDT-позиция — после `representation`.
pub const F_EXT_SHAPE: FieldId = FieldId(123);
/// extInfo Popup: `shapeRepresentation` — представление формы подменю (Enum, симметрично).
/// EDT-позиция — после `shape`.
pub const F_EXT_SHAPE_REPRESENTATION: FieldId = FieldId(124);
/// extInfo UsualGroup: `format` — формат заголовка-данных группы (Localized, Symmetric).
/// Метамодель UsualGroupExtInfo: slaveItemsWidth → **format** → showTitle. Witness —
/// ЗагрузкаКурсовВалют ×2 (EDT united→format→showTitle/throughAlign ⟷ Designer
/// Representation→Format→ShowTitle/ExtendedTooltip).
pub const F_EXT_FORMAT: FieldId = FieldId(125);

// ============================ extInfo: Pages ============================

/// extInfo Pages: `pagesRepresentation` (KEEP: EDT опускает `None`, Designer опускает `Auto`).
pub const F_EXT_PAGES_REPRESENTATION: FieldId = FieldId(131);

// ============================ extInfo: Page ============================

/// extInfo Page: `picture` (Picture-кодек).
pub const F_EXT_PICTURE: FieldId = FieldId(141);

// ============================ extInfo: ButtonGroup ============================

/// extInfo ButtonGroup: `commandSource` — источник команд (`Item.<Имя>`; Ref, симметрично).
pub const F_EXT_COMMAND_SOURCE: FieldId = FieldId(161);

// ============================ extInfo: CommandBar ============================

/// extInfo CommandBar: `appearanceMode` — режим отображения (Enum, симметрично).
pub const F_EXT_APPEARANCE_MODE: FieldId = FieldId(171);

// ============================ extInfo: Popup ============================

/// extInfo Popup: `importance` — важность подменю (KEEP: EDT эмитит ВСЕГДА `Normal`
/// 49/49; Designer НИКОГДА — fill `Normal`).
pub const F_EXT_IMPORTANCE: FieldId = FieldId(172);

/// Designer-fill `importance` Popup — Designer ОПУСКАЕТ свой дефолт `Normal`.
pub const POPUP_IMPORTANCE_FILL: &str = "Normal";
/// EDT-fill `importance` Popup — EDT-ДЕФОЛТ `Main` (та же EMF-конвенция, что у
/// [`super::form_field::TEXT_SIZE_EDT_FILL`]: дефолт EEnum-атрибута = первый литерал).
/// Контингентная таблица (EDT × Designer) по всем 173 Popup'ам SSL ЧИСТАЯ, без внедиагональных
/// клеток: `<ABSENT>`⟺`Main` (1), `Normal`⟺`Normal` (169), `Supplementary`⟺`Supplementary` (3).
/// См. `probe_defaults`.
pub const POPUP_IMPORTANCE_EDT_FILL: &str = "Main";

// ============================ extInfo: ColumnGroup (LANE-F-4) ============================

/// extInfo ColumnGroup: `showInCard` (Bool, симметрично).
pub const F_EXT_SHOW_IN_CARD: FieldId = FieldId(181);
/// extInfo ColumnGroup: `showInHeader` (Bool, противоположные bool-дефолты).
pub const F_EXT_SHOW_IN_HEADER: FieldId = FieldId(182);
/// extInfo ColumnGroup: `headerPicture` (PictureRef).
pub const F_EXT_HEADER_PICTURE: FieldId = FieldId(183);
/// extInfo ColumnGroup: `showTitleInCard` (Bool, симметрично).
pub const F_EXT_SHOW_TITLE_IN_CARD: FieldId = FieldId(184);

/// Designer-дефолт `group` ColumnGroup (омиссия `Vertical`; EDT опускает `Horizontal` —
/// cross-omission 10/5, отличается от UsualGroup, где Designer-дефолт `Auto`).
pub const COLUMN_GROUP_GROUP_DESIGNER_DEFAULT: &str = "Vertical";

/// EDT-дефолт extInfo `horizontalAlign` CommandBar (омиссия `Auto`; Designer-тег
/// `HorizontalLocation`, Designer-дефолт `Left` — cross-omission 16/1).
pub const CB_HORIZONTAL_ALIGN_EDT_DEFAULT: &str = "Auto";
/// Designer-дефолт `HorizontalLocation` CommandBar (омиссия `Left`).
pub const CB_HORIZONTAL_ALIGN_DESIGNER_DEFAULT: &str = "Left";

// -- пер-форматные keep-дефолты (EDT-omit / Designer-omit) --

/// EDT-дефолт extInfo `group` UsualGroup.
pub const GROUP_EDT_DEFAULT: &str = "Horizontal";
/// EDT-дефолт extInfo `behavior`.
pub const BEHAVIOR_EDT_DEFAULT: &str = "Usual";
/// EDT-дефолт extInfo `representation`.
pub const REPRESENTATION_EDT_DEFAULT: &str = "None";
/// EDT-дефолт extInfo `showTitle`.
pub const SHOW_TITLE_EDT_DEFAULT: &str = "false";
/// EDT-дефолт extInfo `throughAlign`.
pub const THROUGH_ALIGN_EDT_DEFAULT: &str = "Use";
/// EDT-дефолт extInfo `pagesRepresentation` (Pages).
pub const PAGES_REPRESENTATION_EDT_DEFAULT: &str = "None";
/// EDT-дефолт extInfo `representation` Popup (EDT опускает `Text`; Designer опускает `Auto`;
/// сверено корпусом покрытия — Popup эмитит `Picture`/`PictureAndText`/`Usual`/`Compact`/`Auto`,
/// а `Text`-вариант пуст в EDT). ПРОТИВОПОЛОЖНЫЕ дефолты (Text ⟺ Auto).
pub const POPUP_REPRESENTATION_EDT_DEFAULT: &str = "Text";
/// Designer-дефолт keep-enum-полей группы (омиссия Auto/auto).
pub const DESIGNER_AUTO: &str = "Auto";
/// Designer-дефолт `showTitle` (lower-case `auto`).
pub const DESIGNER_AUTO_LOWER: &str = "auto";
/// EDT-дефолт extInfo `currentRowUse` UsualGroup (EDT ОПУСКАЕТ `Use`; сверено корпусом
/// покрытия — 78 групп `Auto` + 1 `DontUse` эмитятся, а `Use` опускается). Designer-дефолт —
/// `Auto` ([`DESIGNER_AUTO`]); ПРОТИВОПОЛОЖНЫЕ дефолты (Use ⟺ Auto) — KEEP-пара.
pub const CURRENT_ROW_USE_EDT_DEFAULT: &str = "Use";

// ============================ спеки ============================

fn sym(id: FieldId, name: &'static str, kind: ValueKind) -> FieldSpec {
    FieldSpec::required(id, name, kind)
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
fn en(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Enum)
}
fn bl(id: FieldId, name: &'static str) -> FieldSpec {
    sym(id, name, ValueKind::Bool)
}

/// Общее тело группы в КАНОНИЧЕСКОМ порядке (= EDT-порядок, topo 713 групп, конфликтов 0).
fn build_body_fields() -> Vec<FieldSpec> {
    vec![
        en(F_DISPLAY_IMPORTANCE, "displayImportance"),
        bl(F_VISIBLE, "visible"),
        bl(F_ENABLED, "enabled"),
        bl(F_USER_VISIBLE, "userVisible"),
        loc(F_TITLE, "title"),
        sym(F_TITLE_TEXT_COLOR, "titleTextColor", ValueKind::Ref),
        // EDT-канон: `toolTip` ПЕРЕД `readOnly` (сверено корпусом — UsualGroup
        // «Последнее отправление» в `НастройкиСинхронизацииДанных`; тот же порядок, что у
        // FormField-семейства). Designer эмитит их в собственном порядке (`DES_GROUP_ORDER`).
        loc(F_TOOL_TIP, "toolTip"),
        // toolTipRepresentation СРАЗУ после toolTip (метамодель; SSL toolTip<TTR×27,
        // TTR<enableContentChange×1 — вскрыто РассылкиОтчетов.ПредварительныйПросмотрПисьма).
        en(F_TOOL_TIP_REPRESENTATION, "toolTipRepresentation"),
        bl(F_READ_ONLY, "readOnly"),
        bl(F_ENABLE_CONTENT_CHANGE, "enableContentChange"),
        sym(F_SHORTCUT, "shortcut", ValueKind::Str),
        sym(F_WIDTH, "width", ValueKind::Int),
        sym(F_HEIGHT, "height", ValueKind::Int),
        bl(F_HORIZONTAL_STRETCH, "horizontalStretch"),
        bl(F_VERTICAL_STRETCH, "verticalStretch"),
        en(F_GROUP_HORIZONTAL_ALIGN, "groupHorizontalAlign"),
        en(F_GROUP_VERTICAL_ALIGN, "groupVerticalAlign"),
    ]
}

/// extInfo UsualGroup в каноническом порядке (= EDT extInfo, topo 395/395).
fn build_usual_ext_fields() -> Vec<FieldSpec> {
    vec![
        en(F_EXT_GROUP, "group"),
        en(F_EXT_CHILDREN_ALIGN, "childrenAlign"),
        en(F_EXT_HORIZONTAL_SPACING, "horizontalSpacing"),
        // Порядок align/spacing = МЕТАМОДЕЛЬ UsualGroupExtInfo (horizontalSpacing →
        // verticalSpacing → horizontalAlign → verticalAlign): SSL-витнессы
        // horizontalSpacing<verticalSpacing×12, verticalSpacing<horizontalAlign×12,
        // horizontalAlign<verticalAlign×6; 0 контрпримеров — прежний порядок
        // (verticalAlign до verticalSpacing/horizontalAlign) был tie-break-артефактом,
        // вскрыт РассылкиОтчетов.ПолучателиРассылки после снятия read-блокера.
        en(F_EXT_VERTICAL_SPACING, "verticalSpacing"),
        en(F_EXT_HORIZONTAL_ALIGN, "horizontalAlign"),
        en(F_EXT_VERTICAL_ALIGN, "verticalAlign"),
        en(F_EXT_BEHAVIOR, "behavior"),
        loc(
            F_EXT_COLLAPSED_REPRESENTATION_TITLE,
            "collapsedRepresentationTitle",
        ),
        bl(F_EXT_COLLAPSED, "collapsed"),
        en(F_EXT_CONTROL_REPRESENTATION, "controlRepresentation"),
        bl(F_EXT_SCROLL_ON_COMPRESS, "scrollOnCompress"),
        en(F_EXT_REPRESENTATION, "representation"),
        bl(F_EXT_HYPERLINK, "hyperlink"),
        bl(F_EXT_SHOW_AS_CARD, "showAsCard"),
        bl(F_EXT_SHOW_LEFT_MARGIN, "showLeftMargin"),
        bl(F_EXT_UNITED, "united"),
        sym(F_EXT_BACK_COLOR, "backColor", ValueKind::Ref),
        en(F_EXT_SLAVE_ITEMS_WIDTH, "slaveItemsWidth"),
        loc(F_EXT_FORMAT, "format"),
        en(F_EXT_SHOW_TITLE, "showTitle"),
        sym(F_EXT_TITLE_DATA_PATH, "titleDataPath", ValueKind::Ref),
        en(F_EXT_THROUGH_ALIGN, "throughAlign"),
        en(F_EXT_CURRENT_ROW_USE, "currentRowUse"),
    ]
}

/// extInfo Pages в каноническом порядке (= EDT extInfo; события — каркас).
fn build_pages_ext_fields() -> Vec<FieldSpec> {
    vec![
        en(F_EXT_PAGES_REPRESENTATION, "pagesRepresentation"),
        en(F_EXT_CURRENT_ROW_USE, "currentRowUse"),
    ]
}

/// extInfo Page в каноническом порядке (= EDT extInfo, topo 110/110).
fn build_page_ext_fields() -> Vec<FieldSpec> {
    vec![
        sym(F_EXT_PICTURE, "picture", ValueKind::Ref),
        en(F_EXT_GROUP, "group"),
        en(F_EXT_CHILDREN_ALIGN, "childrenAlign"),
        en(F_EXT_HORIZONTAL_SPACING, "horizontalSpacing"),
        // Порядок = МЕТАМОДЕЛЬ PageGroupExtInfo (verticalSpacing → horizontalAlign →
        // verticalAlign → slaveItemsWidth); SSL horizontalAlign<verticalAlign×2,
        // slaveItemsWidth<showTitle×4, 0 контрпримеров (прежний порядок — tie-break-артефакт).
        en(F_EXT_VERTICAL_SPACING, "verticalSpacing"),
        en(F_EXT_HORIZONTAL_ALIGN, "horizontalAlign"),
        en(F_EXT_VERTICAL_ALIGN, "verticalAlign"),
        en(F_EXT_SLAVE_ITEMS_WIDTH, "slaveItemsWidth"),
        en(F_EXT_SHOW_TITLE, "showTitle"),
        sym(F_EXT_TITLE_DATA_PATH, "titleDataPath", ValueKind::Ref),
        // backColor — метамодель PageGroupExtInfo: после titleDataPath, ДО scrollOnCompress
        // (witnesses УдалениеПомеченныхОбъектов group→backColor→scrollOnCompress,
        // НастройкиОчисткиФайлов showTitle→backColor→scrollOnCompress).
        sym(F_EXT_BACK_COLOR, "backColor", ValueKind::Ref),
        bl(F_EXT_SCROLL_ON_COMPRESS, "scrollOnCompress"),
    ]
}

/// extInfo ButtonGroup в каноническом порядке (= EDT extInfo).
fn build_button_group_ext_fields() -> Vec<FieldSpec> {
    vec![
        sym(F_EXT_COMMAND_SOURCE, "commandSource", ValueKind::Ref),
        en(F_EXT_REPRESENTATION, "representation"),
    ]
}

/// extInfo CommandBar в каноническом порядке (= EDT extInfo, topo 21/21):
/// horizontalAlign (⟺ Designer `HorizontalLocation`), appearanceMode, commandSource.
fn build_command_bar_ext_fields() -> Vec<FieldSpec> {
    vec![
        en(F_EXT_HORIZONTAL_ALIGN, "horizontalAlign"),
        en(F_EXT_APPEARANCE_MODE, "appearanceMode"),
        sym(F_EXT_COMMAND_SOURCE, "commandSource", ValueKind::Ref),
    ]
}

/// extInfo Popup в каноническом порядке (= EDT extInfo, topo 49/49):
/// picture, representation, commandSource, importance.
fn build_popup_ext_fields() -> Vec<FieldSpec> {
    vec![
        sym(F_EXT_PICTURE, "picture", ValueKind::Ref),
        en(F_EXT_REPRESENTATION, "representation"),
        en(F_EXT_SHAPE, "shape"),
        en(F_EXT_SHAPE_REPRESENTATION, "shapeRepresentation"),
        sym(F_EXT_COMMAND_SOURCE, "commandSource", ValueKind::Ref),
        en(F_EXT_IMPORTANCE, "importance"),
    ]
}

/// extInfo ColumnGroup в каноническом порядке (= EDT extInfo = xcore-модель
/// ColumnGroupExtInfo): group, showTitle, showInHeader, headerPicture, showTitleInCard,
/// showInCard (`showInCard` — ПОСЛЕДНИЙ атрибут модели; SSL-витнессы 25×/2×/2× без
/// контр-примеров).
fn build_column_group_ext_fields() -> Vec<FieldSpec> {
    vec![
        en(F_EXT_GROUP, "group"),
        en(F_EXT_SHOW_TITLE, "showTitle"),
        bl(F_EXT_SHOW_IN_HEADER, "showInHeader"),
        sym(F_EXT_HEADER_PICTURE, "headerPicture", ValueKind::Ref),
        bl(F_EXT_SHOW_TITLE_IN_CARD, "showTitleInCard"),
        bl(F_EXT_SHOW_IN_CARD, "showInCard"),
    ]
}

fn leak_spec(entity: &'static str, fields: Vec<FieldSpec>) -> EntitySpec {
    EntitySpec {
        entity,
        fields: Box::leak(fields.into_boxed_slice()),
        children: &[],
    }
}

/// Канонический спек ОБЩЕГО тела группы (кэш на процесс, общий всем видам семейства).
pub fn form_group_body() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("FormGroup", build_body_fields()))
}

fn usual_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("UsualGroupExtInfo", build_usual_ext_fields()))
}
fn pages_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("PagesGroupExtInfo", build_pages_ext_fields()))
}
fn page_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("PageGroupExtInfo", build_page_ext_fields()))
}
fn button_group_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("ButtonGroupExtInfo", build_button_group_ext_fields()))
}
fn command_bar_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("CommandBarExtInfo", build_command_bar_ext_fields()))
}
fn popup_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("PopupGroupExtInfo", build_popup_ext_fields()))
}
fn column_group_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("ColumnGroupExtInfo", build_column_group_ext_fields()))
}

/// Канонический [`ControlSpec`] вида `UsualGroup` (контейнер).
pub fn usual_group() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "UsualGroup",
        properties: form_group_body(),
        ext_info: usual_ext_spec(),
        container: true,
    })
}

/// Канонический [`ControlSpec`] вида `Pages` (контейнер страниц).
pub fn pages() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "Pages",
        properties: form_group_body(),
        ext_info: pages_ext_spec(),
        container: true,
    })
}

/// Канонический [`ControlSpec`] вида `Page` (страница).
pub fn page() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "Page",
        properties: form_group_body(),
        ext_info: page_ext_spec(),
        container: true,
    })
}

/// Канонический [`ControlSpec`] вида `ButtonGroup` (группа кнопок; EDT БЕЗ `<type>`).
pub fn button_group() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "ButtonGroup",
        properties: form_group_body(),
        ext_info: button_group_ext_spec(),
        container: true,
    })
}

/// Канонический [`ControlSpec`] вида `CommandBar` (командная панель-группа).
pub fn command_bar() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "CommandBar",
        properties: form_group_body(),
        ext_info: command_bar_ext_spec(),
        container: true,
    })
}

/// Канонический [`ControlSpec`] вида `Popup` (подменю-группа).
pub fn popup() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "Popup",
        properties: form_group_body(),
        ext_info: popup_ext_spec(),
        container: true,
    })
}

/// Канонический [`ControlSpec`] вида `ColumnGroup` (группа колонок Таблицы; контейнер;
/// EDT — `<items xsi:type="form:FormGroup"><type>ColumnGroup`).
pub fn column_group() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "ColumnGroup",
        properties: form_group_body(),
        ext_info: column_group_ext_spec(),
        container: true,
    })
}
