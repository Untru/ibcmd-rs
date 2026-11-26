//! Канонический спек контрола `Button` (кнопка) — ARCHITECTURE.md §1.4/§1.6, под-IR L1f.
//! Лист (рекурсивных детей не несёт). Срез LANE-F-2: ПОЛНОЕ тело по корпусу (817 кнопок
//! SSL CommonForms, cross-omission witness), таблично-управляемая проекция
//! (`formats-xml/form/tables.rs`).
//!
//! EDT кодирует кнопку как `<items xsi:type="form:Button">`; ВИД кнопки — ДАННЫЕ-поле
//! `<type>` с EDT-дефолтом `CommandBarButton` (омиссия; 762/817) и литералами
//! `UsualButton`(32)/`Hyperlink`(19)/`CommandBarHyperlink`(4); Designer — элемент `<Button>`
//! с `<Type>` ВСЕГДА (817/817). Старая модель (write-константа `CommandBarButton`)
//! корпусом ОПРОВЕРГНУТА и заменена keep-полем.
//!
//! # Ключевые пер-форматные дефолты (cross-omission)
//! * `buttonImportance` — ТОЧНОЕ ПРИСУТСТВИЕ (омиссия ⇒ отсутствие в bag у ОБОИХ ридеров);
//!   омиссии диалектов НЕ дизъюнктны, полный кросс-витнесс-ценз по трём конфигурациям — в доке
//!   [`BUTTON_IMPORTANCE_DESIGNER_DEFAULT`].
//! * `representation` — EDT опускает `Text` (14), Designer опускает `Auto` (656+).
//! * `placementArea`/`representationInContextMenu` — EDT эмитит ВСЕГДА (`UserCmds`/`Auto`
//!   817/817); Designer НИКОГДА/опускает `Auto`.
//! * `autoMaxWidth`/`autoMaxHeight` — противоположные bool-дефолты (EDT true / Designer false).
//! * `enabled` — EDT всегда (814 true + 3 false); Designer опускает true.
//! * `font` — НЕ смоделирован (композит из 15 кнопок; §1.0-громко).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::spec::forms::controls::ControlSpec;

/// `title` — локализованный заголовок кнопки.
pub const F_TITLE: FieldId = FieldId(1);
/// `visible` — видимость (противоположные bool-дефолты).
pub const F_VISIBLE: FieldId = FieldId(2);
/// `enabled` — доступность (EDT всегда; Designer опускает true).
pub const F_ENABLED: FieldId = FieldId(3);
/// `userVisible` — пользовательская видимость (CommonBool-кодировка).
pub const F_USER_VISIBLE: FieldId = FieldId(4);
/// `defaultItem` — кнопка по умолчанию в панели (Bool, симметрично).
pub const F_DEFAULT_ITEM: FieldId = FieldId(5);
/// `commandName` — имя команды (ref-строка; оба эмитят).
pub const F_COMMAND_NAME: FieldId = FieldId(6);
/// `parameter`/`Parameter` — командный параметр-ссылка (метамодель Button: commandName →
/// **parameter** → buttonImportance; SSL ×6: EDT `core:ReferenceValue`+`<value>` ⟺ Designer
/// `xr:MDObjectRef`-текст; канон Ref).
pub const F_PARAMETER: FieldId = FieldId(39);
/// `representation` — представление кнопки (KEEP: EDT опускает `Text`, Designer — `Auto`).
pub const F_REPRESENTATION: FieldId = FieldId(7);
/// `defaultButton` — кнопка-умолчание формы (Bool, симметрично).
pub const F_DEFAULT_BUTTON: FieldId = FieldId(8);
/// `autoMaxWidth` — авто-максимум ширины (противоположные bool-дефолты).
pub const F_AUTO_MAX_WIDTH: FieldId = FieldId(9);
/// `autoMaxHeight` — авто-максимум высоты (противоположные bool-дефолты).
pub const F_AUTO_MAX_HEIGHT: FieldId = FieldId(10);
/// `placementArea` — область размещения (KEEP: EDT всегда `UserCmds`; Designer никогда).
pub const F_PLACEMENT_AREA: FieldId = FieldId(11);
/// `representationInContextMenu` (KEEP: EDT всегда; Designer опускает `Auto`).
pub const F_REPRESENTATION_IN_CONTEXT_MENU: FieldId = FieldId(12);
/// `buttonImportance` — важность кнопки (ТОЧНОЕ ПРИСУТСТВИЕ: ни один ридер НЕ подставляет
/// fill — см. ценз в [`BUTTON_IMPORTANCE_DESIGNER_DEFAULT`]).
pub const F_BUTTON_IMPORTANCE: FieldId = FieldId(13);
/// `type`/`Type` — ВИД кнопки (KEEP: EDT опускает `CommandBarButton`, Designer эмитит всегда).
pub const F_BUTTON_TYPE: FieldId = FieldId(14);
/// `displayImportance` — важность отображения (EDT элемент / Designer атрибут).
pub const F_DISPLAY_IMPORTANCE: FieldId = FieldId(15);
/// `skipOnInput` — пропускать при вводе (Bool, симметрично; эмитятся явные true/false).
pub const F_SKIP_ON_INPUT: FieldId = FieldId(16);
/// `width` — ширина (Int, симметрично).
pub const F_WIDTH: FieldId = FieldId(17);
/// `height` — высота (Int, симметрично).
pub const F_HEIGHT: FieldId = FieldId(18);
/// `maxWidth` — макс. ширина (Int, симметрично; метамодель после `autoMaxWidth`, до `height`).
pub const F_MAX_WIDTH: FieldId = FieldId(37);
/// `maxHeight` — макс. высота (Int, симметрично).
pub const F_MAX_HEIGHT: FieldId = FieldId(19);
/// `groupHorizontalAlign` (Enum, симметрично).
pub const F_GROUP_HORIZONTAL_ALIGN: FieldId = FieldId(20);
/// `groupVerticalAlign` (Enum, симметрично).
pub const F_GROUP_VERTICAL_ALIGN: FieldId = FieldId(21);
/// `horizontalStretch` (Bool, симметрично).
pub const F_HORIZONTAL_STRETCH: FieldId = FieldId(22);
/// `verticalStretch` (Bool, симметрично).
pub const F_VERTICAL_STRETCH: FieldId = FieldId(23);
/// `check` — кнопка-переключатель нажата (Bool, симметрично).
pub const F_CHECK: FieldId = FieldId(24);
/// `picture` — картинка кнопки (Picture-кодек).
pub const F_PICTURE: FieldId = FieldId(25);
/// `textColor` — цвет текста (Color-кодек).
pub const F_TEXT_COLOR: FieldId = FieldId(26);
/// `borderColor` — цвет рамки (Color-кодек).
pub const F_BORDER_COLOR: FieldId = FieldId(27);
/// `toolTipRepresentation` (Enum, симметрично).
pub const F_TOOL_TIP_REPRESENTATION: FieldId = FieldId(28);
/// `shapeRepresentation` — представление фигуры (Enum, симметрично).
pub const F_SHAPE_REPRESENTATION: FieldId = FieldId(29);
/// `locationInCommandBar` — размещение в командной панели (Enum, симметрично).
pub const F_LOCATION_IN_COMMAND_BAR: FieldId = FieldId(30);
/// `shape` — форма кнопки (Enum, симметрично: `Usual`/`Oval`/…; оба формата эмитят
/// не-дефолтное значение, оба опускают дефолт — сверено корпусом покрытия). EDT-позиция —
/// после `representationInContextMenu`; Designer — сразу после `<Title>`.
pub const F_SHAPE: FieldId = FieldId(31);
/// `pictureLocation` — расположение картинки (Enum, симметрично: `Left`/`Right`/…; оба
/// эмитят не-дефолт, оба опускают дефолт — сверено корпусом). EDT — после `shape`; Designer —
/// после `<Title>`.
pub const F_PICTURE_LOCATION: FieldId = FieldId(32);
/// `commandUniqueness` — уникальность команды (Bool, симметрично: оба эмитят `false`, оба
/// опускают дефолт true — сверено корпусом). EDT — после `pictureLocation`; Designer — после `<Title>`.
pub const F_COMMAND_UNIQUENESS: FieldId = FieldId(33);
/// `showAsCard` — показывать как карточку (Bool, симметрично: оба эмитят `true`, оба опускают
/// дефолт false). EDT — после `commandUniqueness`; Designer — после `<Title>`.
pub const F_SHOW_AS_CARD: FieldId = FieldId(34);
/// `dataPath` — путь данных ДАННЫЕ-ПРИВЯЗАННОЙ кнопки (DataPath dual-encoding, как у
/// FormField; Symmetric — SSL 52⟷52). Метамодель `Button`: `userVisible` (11) →
/// **dataPath (14)** → `defaultItem` (15) → `skipOnInput` (16); EDT-витнессы (52/52):
/// `userVisible → dataPath → [skipOnInput] → extendedTooltip`; Designer — после
/// `CommandName`, до `Title`/`LocationInCommandBar` (witness `CommandName > DataPath > Title`).
pub const F_DATA_PATH: FieldId = FieldId(35);
/// `titleHeight` — высота заголовка кнопки (Int, симметрично; метамодель `Button` после
/// `titleLocation`, до `shortcut`; в HEAD-регионе — до `extendedTooltip`/`type`).
pub const F_TITLE_HEIGHT: FieldId = FieldId(36);
/// `backColor` — цвет фона (Color-кодек: ColorRef ИЛИ ColorDef-триплет ⟺ Designer
/// `style:`/`pal:`/`#RRGGBB`; Symmetric). Метамодель Button: textColor(43) → **backColor(44)**
/// → borderColor(45) → font → picture; корпус: placementArea<backColor×10, textColor<
/// backColor×9, backColor<borderColor×9, backColor<picture×1 (witness
/// ПомощникСозданияОбменаДанными.НастройкаСинхронизации #FFFFFF).
pub const F_BACK_COLOR: FieldId = FieldId(38);

/// ПЛАТФОРМЕННЫЙ дефолт `buttonImportance` — литерал, который Designer-дамп ОПУСКАЕТ ВСЕГДА
/// (контекст-независимо), и который платформа кладёт в cf-ячейку `[57]` ординалом `1`.
/// В КАНОН НЕ ХРАНИТСЯ (свойство не задано ⇒ отсутствие в bag у ОБОИХ ридеров).
///
/// # КРОСС-ВИТНЕСС-ЦЕНЗ (r35, ТРИ конфигурации, 84 496 кнопок, join по (форма, name, id))
/// Скрипт — сопоставление КАЖДОЙ кнопки в ДВУХ дампах ОДНОЙ конфигурации:
///
/// | конфигурация (компат / писатель) | Designer | EDT | defaultButton | кнопок |
/// |---|---|---|---|---:|
/// | SSL 8.5.1 / 8.5.1   | ОПУСКАЕТ        | `Normal`        | –   | 3 554 |
/// | SSL 8.5.1 / 8.5.1   | **`Main`**      | **ОПУСКАЕТ**    | да  |   471 |
/// | SSL 8.5.1 / 8.5.1   | `Supplementary` | `Supplementary` | –   |   158 |
/// | coverage 8.3.20 / 8.5.1 | ОПУСКАЕТ    | `Normal`        | –   |    60 |
/// | coverage 8.3.20 / 8.5.1 | ОПУСКАЕТ    | ОПУСКАЕТ        | да  |     1 |
/// | ERP 8.3.27 / 8.3.27 | ОПУСКАЕТ        | ОПУСКАЕТ        | –   | 74 071 |
/// | ERP 8.3.27 / 8.3.27 | ОПУСКАЕТ        | ОПУСКАЕТ        | да  |  6 181 |
///
/// Дискриминирующих пар (один диалект ЯВНО, другой ОПУСКАЕТ): SSL 2 клетки / 4 025 кнопок,
/// coverage 1 клетка / 60, ERP **0**. Контрольный сплошной поиск по ВСЕМУ корпусу ERP:
/// подстрока `ButtonImportance` — **0** вхождений, `buttonImportance` — **0**.
///
/// # ЗАКОНЫ, КОТОРЫЕ ЭТО ДАЁТ
/// * `Normal` = дефолт ПЛАТФОРМЫ: Designer опускает его 3 554/3 554 (SSL) + 60/60 (coverage) —
///   там EDT говорит `Normal` ЯВНО. Designer НЕ эмитит `Normal` НИ РАЗУ ни в одном корпусе.
/// * `Main` У КНОПКИ-УМОЛЧАНИЯ — ПРОИЗВОДНОЕ, а не заданное значение: при компате ≥ 8.5
///   платформа сама делает важность default-кнопки `Main` (SSL: Designer пишет `Main`
///   471/471, оракул хранит `[57] = 0`), при компате < 8.5 — оставляет `Normal`
///   (coverage: Designer ОПУСКАЕТ, `[57] = 1`). Это ровно
///   `FormBodyProfile::default_button_importance_zero` — теперь ОБЪЯСНЁННЫЙ, а не эвристика.
/// * Под компатом 8.3.27 свойства НЕТ ВООБЩЕ: ни один из 80 252 носителей ERP не имеет тега
///   ни в одном диалекте, `Supplementary` не встречается ни разу (при 3.8 % на SSL). В
///   V50-записи Button ячейки `[57]` тоже нет (`relane_v50_record`, K = 7).
/// * ⇒ **ОМИССИИ ДИАЛЕКТОВ НЕ ДИЗЪЮНКТНЫ** (в отличие от `editMode`/`headerHorizontalAlign`,
///   см. `Policy::PlatformDefault`): ERP даёт 80 252 клетки «ОБА ОПУСКАЮТ». Значит НИКАКОЙ
///   fill-литерал не восстановим из источника — любой был бы ДОМЫСЛОМ (§1.0). Канон = ТОЧНОЕ
///   ПРИСУТСТВИЕ: опущено ⇒ отсутствует в bag (оба ридера), явное ⇒ хранится как есть.
///
/// # ЧТО ОПРОВЕРГНУТО
/// Прежняя КОНТЕКСТ-ЗАВИСИМАЯ схема (`BUTTON_IMPORTANCE_EDT_DEFAULT = "Main"` у обычной кнопки /
/// `"Normal"` у кнопки-умолчания) была выведена из ЕДИНСТВЕННОЙ кнопки corpus'а покрытия и
/// ИНВЕРТИРОВАНА относительно источника: у кнопки-умолчания EDT-омиссия значит `Main`
/// (471/471 SSL), а не `Normal`. Одновременно `"Main"` у обычной кнопки — чистая фабрикация:
/// все 74 071 обычных ERP-кнопок, где EDT опускает тег, Designer тоже опускает. Именно эта
/// фабрикация роняла `edt→cf` на ERP отказом `Button.buttonImportance literal "Main"
/// unwitnessed` на ПЕРВОЙ же форме.
pub const BUTTON_IMPORTANCE_DESIGNER_DEFAULT: &str = "Normal";
/// EDT-дефолт `type` (EDT ОПУСКАЕТ `CommandBarButton`; Designer эмитит всегда).
pub const BUTTON_TYPE_EDT_DEFAULT: &str = "CommandBarButton";
/// EDT-дефолт `representation` (EDT ОПУСКАЕТ `Text`).
pub const REPRESENTATION_EDT_DEFAULT: &str = "Text";
/// Designer-дефолт `representation` (Designer ОПУСКАЕТ `Auto`).
pub const REPRESENTATION_DESIGNER_DEFAULT: &str = "Auto";
/// Fill `placementArea` (EDT-эмитимая константа корпуса 817/817).
pub const PLACEMENT_AREA_FILL: &str = "UserCmds";
/// Designer-дефолт `representationInContextMenu` (Designer омитит `Auto`).
pub const REPRESENTATION_IN_CONTEXT_MENU_DEFAULT: &str = "Auto";
/// EDT-дефолт `representationInContextMenu` (EDT омитит `None`; сверено корпусом покрытия —
/// кнопка `ОтображениеВКонтекстномМеню_Нет` пуста в EDT, а `Auto` эмитится). ПРОТИВОПОЛОЖНЫЕ
/// дефолты (None ⟺ Auto) — KEEP-пара.
pub const REPRESENTATION_IN_CONTEXT_MENU_EDT_DEFAULT: &str = "None";

fn sym(id: FieldId, name: &'static str, kind: ValueKind) -> FieldSpec {
    FieldSpec::required(id, name, kind)
}

/// Свойства кнопки в КАНОНИЧЕСКОМ порядке (= EDT-порядок тела, topo 817/817, конфликтов 0):
/// displayImportance, title, visible, enabled, userVisible, defaultItem, skipOnInput,
/// [extendedTooltip/contextMenu — каркас], type, commandName, buttonImportance,
/// representation, defaultButton, width, autoMaxWidth, height, autoMaxHeight,
/// groupHorizontalAlign, horizontalStretch, maxHeight, verticalStretch, groupVerticalAlign,
/// placementArea, check, [font — не смоделирован], picture, textColor, borderColor,
/// toolTipRepresentation, representationInContextMenu, shapeRepresentation,
/// locationInCommandBar.
fn build_property_fields() -> Vec<FieldSpec> {
    vec![
        sym(F_DISPLAY_IMPORTANCE, "displayImportance", ValueKind::Enum),
        FieldSpec::with_default(
            F_TITLE,
            "title",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        sym(F_VISIBLE, "visible", ValueKind::Bool),
        sym(F_ENABLED, "enabled", ValueKind::Bool),
        sym(F_USER_VISIBLE, "userVisible", ValueKind::Bool),
        sym(F_DATA_PATH, "dataPath", ValueKind::Ref),
        sym(F_DEFAULT_ITEM, "defaultItem", ValueKind::Bool),
        sym(F_SKIP_ON_INPUT, "skipOnInput", ValueKind::Bool),
        sym(F_TITLE_HEIGHT, "titleHeight", ValueKind::Int),
        sym(F_BUTTON_TYPE, "type", ValueKind::Enum),
        sym(F_COMMAND_NAME, "commandName", ValueKind::Ref),
        sym(F_PARAMETER, "parameter", ValueKind::Ref),
        sym(F_BUTTON_IMPORTANCE, "buttonImportance", ValueKind::Enum),
        sym(F_REPRESENTATION, "representation", ValueKind::Enum),
        sym(F_DEFAULT_BUTTON, "defaultButton", ValueKind::Bool),
        sym(F_WIDTH, "width", ValueKind::Int),
        sym(F_AUTO_MAX_WIDTH, "autoMaxWidth", ValueKind::Bool),
        sym(F_MAX_WIDTH, "maxWidth", ValueKind::Int),
        sym(F_HEIGHT, "height", ValueKind::Int),
        sym(F_AUTO_MAX_HEIGHT, "autoMaxHeight", ValueKind::Bool),
        sym(
            F_GROUP_HORIZONTAL_ALIGN,
            "groupHorizontalAlign",
            ValueKind::Enum,
        ),
        sym(F_HORIZONTAL_STRETCH, "horizontalStretch", ValueKind::Bool),
        sym(F_MAX_HEIGHT, "maxHeight", ValueKind::Int),
        sym(F_VERTICAL_STRETCH, "verticalStretch", ValueKind::Bool),
        sym(
            F_GROUP_VERTICAL_ALIGN,
            "groupVerticalAlign",
            ValueKind::Enum,
        ),
        sym(F_PLACEMENT_AREA, "placementArea", ValueKind::Enum),
        sym(F_CHECK, "check", ValueKind::Bool),
        // Порядок цветов/картинки = метамодель Button (textColor→backColor→borderColor→picture);
        // корпус: textColor<backColor×9, backColor<borderColor×9, borderColor<picture×1,
        // backColor<picture×1 — ПРОТИВОРЕЧИЙ 0 (прежняя позиция picture ДО цветов была слепой).
        sym(F_TEXT_COLOR, "textColor", ValueKind::Ref),
        sym(F_BACK_COLOR, "backColor", ValueKind::Ref),
        sym(F_BORDER_COLOR, "borderColor", ValueKind::Ref),
        sym(F_PICTURE, "picture", ValueKind::Ref),
        sym(
            F_TOOL_TIP_REPRESENTATION,
            "toolTipRepresentation",
            ValueKind::Enum,
        ),
        sym(
            F_REPRESENTATION_IN_CONTEXT_MENU,
            "representationInContextMenu",
            ValueKind::Enum,
        ),
        sym(F_SHAPE, "shape", ValueKind::Enum),
        sym(F_PICTURE_LOCATION, "pictureLocation", ValueKind::Enum),
        sym(F_COMMAND_UNIQUENESS, "commandUniqueness", ValueKind::Bool),
        sym(F_SHOW_AS_CARD, "showAsCard", ValueKind::Bool),
        sym(
            F_SHAPE_REPRESENTATION,
            "shapeRepresentation",
            ValueKind::Enum,
        ),
        sym(
            F_LOCATION_IN_COMMAND_BAR,
            "locationInCommandBar",
            ValueKind::Enum,
        ),
    ]
}

fn properties_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Button",
        fields: Box::leak(build_property_fields().into_boxed_slice()),
        children: &[],
    })
}

/// Пустой extInfo-спек (кнопка extInfo-региона не несёт).
fn ext_info_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ButtonExtInfo",
        fields: &[],
        children: &[],
    })
}

/// Канонический [`ControlSpec`] вида `Button` (лист).
pub fn button() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "Button",
        properties: properties_spec(),
        ext_info: ext_info_spec(),
        container: false,
    })
}
