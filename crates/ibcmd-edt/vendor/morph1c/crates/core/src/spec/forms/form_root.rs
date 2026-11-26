//! Канонический спек ФОРМ-УРОВНЕВЫХ свойств (`<form:Form>` EDT / `<Form>` Designer) —
//! ARCHITECTURE.md §1.4/§1.6, под-IR L1f.
//!
//! Спек несёт РОВНО те форм-атрибуты, что эмитит пилот
//! `CommonForm.ФормаПроизвольногоСообщения` (в EDT ИЛИ Designer). Канонику (id/value_kind/
//! порядок эмиссии) держит ЭТОТ спек; ПЕР-ФОРМАТНЫЕ дефолты (EDT и Designer несут
//! ПРОТИВОПОЛОЖНЫЕ — сверено по 104 CommonForms) живут в проекциях через
//! [`crate::engine::Projection::field_default`] (§1.6). Идентичность формы — в каркасе
//! коннектора.
//!
//! # Пер-форматный default-профиль (сверено crossом по корпусу)
//! Для семейства auto/save/enable EDT-дефолт=`false`, Designer-дефолт=`true` (ПРОТИВОПОЛОЖНЫ):
//! `saveWindowSettings`/`autoUrl`/`autoTitle`/`autoFillCheck`/`allowFormCustomize`.
//! `group`: EDT-дефолт=`Horizontal`, Designer-дефолт=`Auto`. `enabled`/`showTitle`/
//! `showCloseButton`: канон-`required` (нет канонического дефолта ⇒ значение всегда в bag),
//! Designer-дефолт=`true`/`auto`/`true`, EDT-дефолт=`false`/`false`/`false` — ОБА подаются
//! проекцией. ⚠ Прежняя формулировка «EDT эмитит ВСЕГДА» ОПРОВЕРГНУТА кросс-витнесс-цензом:
//! EDT опускает `false` (`enabled` — SSL 1, ERP 5; `showCloseButton` — SSL 1, ERP 36), и ровно
//! на этих формах Designer пишет `false` ЯВНО (клеток «оба опускают» НЕТ ни в одном корпусе).
//! `commandBarLocation`/`showCommandBar`/`windowOpeningMode`/`windowViewMode`: общий дефолт
//! у обоих (Designer не переопределяет). Канонический `fs.default` = EDT-дефолту (он задаёт
//! разрежённость bag); Designer-дефолт подаётся проекцией.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};

/// `commandBarLocation`/`CommandBarLocation` — расположение командной панели (Enum).
pub const F_COMMAND_BAR_LOCATION: FieldId = FieldId(1);
/// `showCommandBar`/`ShowCommandBar` — показывать командную панель (Bool).
pub const F_SHOW_COMMAND_BAR: FieldId = FieldId(2);
/// `windowOpeningMode`/`WindowOpeningMode` — режим открытия окна (Enum). Дефолт `Auto`
/// (общий у обоих; корпус покрытия опускает его — см. [`WINDOW_OPENING_MODE_AUTO`]).
pub const F_WINDOW_OPENING_MODE: FieldId = FieldId(3);
/// `windowViewMode`/`WindowViewMode` — режим показа окна (Enum). Designer несёт
/// `xsi:type="lf:FormWindowViewMode"` на узле.
pub const F_WINDOW_VIEW_MODE: FieldId = FieldId(4);
/// `saveWindowSettings`/`SaveWindowSettings` — сохранять настройки окна (Bool).
pub const F_SAVE_WINDOW_SETTINGS: FieldId = FieldId(5);
/// `autoUrl`/`AutoURL` — авто-навигационная ссылка (Bool).
pub const F_AUTO_URL: FieldId = FieldId(6);
/// `autoTitle`/`AutoTitle` — авто-заголовок (Bool).
pub const F_AUTO_TITLE: FieldId = FieldId(7);
/// `group`/`Group` — направление группировки (Enum).
pub const F_GROUP: FieldId = FieldId(8);
/// `autoFillCheck`/`AutoFillCheck` — авто-проверка заполнения (Bool).
pub const F_AUTO_FILL_CHECK: FieldId = FieldId(9);
/// `allowFormCustomize`/`Customizable` — настраиваемость формы (Bool).
pub const F_ALLOW_FORM_CUSTOMIZE: FieldId = FieldId(10);
/// `enabled`/`Enabled` — доступность формы (Bool). Канон-`required` (значение ВСЕГДА в bag);
/// омиссию каждый диалект трактует по-своему — EDT `false`, Designer `true` (проекция).
pub const F_ENABLED: FieldId = FieldId(11);
/// `showTitle`/`ShowTitle` — показывать заголовок (Enum). Канон-`required`; EDT-омиссия ⇒
/// `false`, Designer-омиссия ⇒ `auto` (проекция).
pub const F_SHOW_TITLE: FieldId = FieldId(12);
/// `showCloseButton`/`ShowCloseButton` — показывать кнопку закрытия (Bool). Канон-`required`;
/// EDT-омиссия ⇒ `false`, Designer-омиссия ⇒ `true` (проекция).
pub const F_SHOW_CLOSE_BUTTON: FieldId = FieldId(13);
/// `width` — ширина формы (Int; оба формата эмитят, симметрично).
pub const F_WIDTH: FieldId = FieldId(14);
/// `autoSaveDataInSettings` — автосохранение данных в настройках (Enum, симметрично).
pub const F_AUTO_SAVE_DATA_IN_SETTINGS: FieldId = FieldId(15);
/// `enterKeyBehavior` — поведение Enter (Enum, симметрично).
pub const F_ENTER_KEY_BEHAVIOR: FieldId = FieldId(16);
/// `saveDataInSettings` — сохранение данных в настройках (Enum, симметрично).
pub const F_SAVE_DATA_IN_SETTINGS: FieldId = FieldId(17);
/// `horizontalAlign` — горизонтальное выравнивание формы (Enum, симметрично).
pub const F_HORIZONTAL_ALIGN: FieldId = FieldId(18);
/// `verticalScroll` — вертикальная прокрутка (Enum; ЛИТЕРАЛЫ РАСХОДЯТСЯ регистром:
/// EDT `UseIfNecessary` ⟺ Designer `useIfNecessary` — EnumMap в проекции).
pub const F_VERTICAL_SCROLL: FieldId = FieldId(19);
/// `conversationsRepresentation` — представление обсуждений (Enum, симметрично).
pub const F_CONVERSATIONS_REPRESENTATION: FieldId = FieldId(20);
/// `height`/`Height` — высота формы (Int, симметрично; оба формата эмитят фактическое
/// значение, оба опускают дефолт `0`). EDT-позиция — сразу после `width`.
pub const F_HEIGHT: FieldId = FieldId(21);
/// `verticalAlign`/`VerticalAlign` — вертикальное выравнивание формы-контейнера (Enum,
/// симметрично; оба опускают дефолт `Auto`, эмитят лишь не-дефолт — witness `Bottom`).
pub const F_VERTICAL_ALIGN: FieldId = FieldId(22);
/// `horizontalSpacing`/`HorizontalSpacing` — горизонтальный интервал детей формы-контейнера
/// (Enum, симметрично; оба опускают дефолт `Auto` — witness `Double`).
pub const F_HORIZONTAL_SPACING: FieldId = FieldId(23);
/// `childItemsWidth`/`ChildItemsWidth` — ширина подчинённых элементов формы-контейнера
/// (Enum, симметрично; оба опускают дефолт `Auto` — witness `LeftWide`).
pub const F_CHILD_ITEMS_WIDTH: FieldId = FieldId(24);
/// `scale`/`Scale` — масштаб формы (Int; канон-дефолт 0 = не задан, общая омиссия). EDT
/// кодирует ДЕСЯТИЧНЫМ литералом `101.0` ⟺ Designer целым `101` (witness
/// ПомощникСозданияОбменаДанными.ВыборТипаТранспорта — единственный носитель корпуса).
pub const F_SCALE: FieldId = FieldId(25);
/// `verticalSpacing`/`VerticalSpacing` — вертикальный интервал детей формы-контейнера
/// (Enum, симметрично; оба опускают дефолт `Auto` — witness `Half`,
/// УничтожениеПерсональныхДанных.ФормаСозданияАктов: EDT group→verticalSpacing→autoFillCheck
/// ⟺ Designer Group→VerticalSpacing→CommandBarLocation).
pub const F_VERTICAL_SPACING: FieldId = FieldId(26);
// ERP-witnessed корневые форм-свойства (M-волна 2: root-{59}-регион; designer-счётчики
// ScalingMode×190 / CollapseItemsByImportanceVariant×130 / ChildrenAlign×107 + GroupList /
// SettingsStorage): всё опциональные (present ⟺ недефолт).
/// `scalingMode` ⟺ `ScalingMode` (enum).
pub const F_SCALING_MODE: FieldId = FieldId(27);
/// `collapseItemsByImportanceVariant` ⟺ `CollapseItemsByImportanceVariant` (enum).
pub const F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT: FieldId = FieldId(28);
/// `childrenAlign` ⟺ `ChildrenAlign` (enum).
pub const F_CHILDREN_ALIGN: FieldId = FieldId(29);
/// `groupList` ⟺ `GroupList` (путь-текст).
pub const F_GROUP_LIST: FieldId = FieldId(30);
/// `settingsStorage` ⟺ `SettingsStorage` (ссылка-текст).
pub const F_SETTINGS_STORAGE: FieldId = FieldId(31);

/// Канонический литерал `commandBarLocation = Auto` (общий дефолт).
pub const COMMAND_BAR_LOCATION_AUTO: &str = "Auto";
/// Канонический литерал `showCommandBar = auto` (ТРИ-состояние: EDT/Designer эмитят
/// `true`/`false` явно, ОПУСКАЮТ неявный `auto` — сверено 51 true + 4 false + 49 omit).
pub const SHOW_COMMAND_BAR_AUTO: &str = "auto";
/// Канонический литерал `windowViewMode = Auto` (общий дефолт).
pub const WINDOW_VIEW_MODE_AUTO: &str = "Auto";
/// Канонический литерал `windowOpeningMode = Auto` (общий дефолт у обоих форматов; сверено
/// корпусом покрытия s10_forms: все 10 форм ОПУСКАЮТ его в EDT И Designer ⇒ он не-required
/// с дефолтом `Auto`, эмитится обоими форматами лишь при не-дефолтном значении).
pub const WINDOW_OPENING_MODE_AUTO: &str = "Auto";
/// Канонический литерал `group = Horizontal` (EDT-дефолт = канонический).
pub const GROUP_EDT_DEFAULT: &str = "Horizontal";

/// Форм-уровневые поля в каноническом порядке эмиссии (= порядок EDT-файла).
///
/// Канонический `fs.default` = EDT-ДЕФОЛТ (определяет разрежённость bag); пер-форматный
/// Designer-дефолт подаётся проекцией (`field_default`). Поля `enabled`/`showTitle`/
/// `showCloseButton` — `required` (EDT эмитит всегда), Designer-дефолт у них в проекции.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_COMMAND_BAR_LOCATION,
            "commandBarLocation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(COMMAND_BAR_LOCATION_AUTO)),
        ),
        // showCommandBar — ТРИ-состояние {auto(омитится), true, false}: канон-дефолт `auto`,
        // так `true` И `false` — не-дефолт (оба формата эмитят их явно; `auto` опускают оба).
        FieldSpec::with_default(
            F_SHOW_COMMAND_BAR,
            "showCommandBar",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(SHOW_COMMAND_BAR_AUTO)),
        ),
        // windowOpeningMode: НЕ required — общий дефолт `Auto` у обоих форматов (корпус
        // покрытия опускает его целиком; SSL-формы эмитили не-дефолт → он в bag только тогда).
        FieldSpec::with_default(
            F_WINDOW_OPENING_MODE,
            "windowOpeningMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(WINDOW_OPENING_MODE_AUTO)),
        ),
        FieldSpec::with_default(
            F_WINDOW_VIEW_MODE,
            "windowViewMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(WINDOW_VIEW_MODE_AUTO)),
        ),
        FieldSpec::with_default(
            F_SAVE_WINDOW_SETTINGS,
            "saveWindowSettings",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_AUTO_URL,
            "autoUrl",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_AUTO_TITLE,
            "autoTitle",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_GROUP,
            "group",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(GROUP_EDT_DEFAULT)),
        ),
        FieldSpec::with_default(
            F_AUTO_FILL_CHECK,
            "autoFillCheck",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_ALLOW_FORM_CUSTOMIZE,
            "allowFormCustomize",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // enabled/showTitle/showCloseButton: `required` = НЕТ канонического дефолта ⇒ значение
        // никогда не опускается из bag. Пер-форматные дефолты ОБА в проекции: Designer
        // true/auto/true, EDT false/false/false (омиссии диалектов ДИЗЪЮНКТНЫ — кросс-витнесс-ценз
        // по SSL 876/876, ERP 11 400/11 400, coverage 28/28, клеток «оба опускают» НЕТ).
        FieldSpec::required(F_ENABLED, "enabled", ValueKind::Bool),
        FieldSpec::required(F_SHOW_TITLE, "showTitle", ValueKind::Enum),
        FieldSpec::required(F_SHOW_CLOSE_BUTTON, "showCloseButton", ValueKind::Bool),
        // Симметричные root-атрибуты LANE-F-2 (оба формата эмитят не-дефолт; сверено
        // cross-omission по корпусу; канон-дефолт = общая омиссия).
        FieldSpec::with_default(F_WIDTH, "width", ValueKind::Int, PropertyValue::Int(0)),
        FieldSpec::with_default(
            F_AUTO_SAVE_DATA_IN_SETTINGS,
            "autoSaveDataInSettings",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
        FieldSpec::with_default(
            F_ENTER_KEY_BEHAVIOR,
            "enterKeyBehavior",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("ControlsTraversal")),
        ),
        FieldSpec::with_default(
            F_SAVE_DATA_IN_SETTINGS,
            "saveDataInSettings",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
        // horizontalAlign: канон-дефолт `Auto` (= отсутствие тега). ОБА формата эмитят явные
        // `Left`/`Center` (SSL ×3+×3; напр. ЗадачаИсполнителя.ФормаЗадачи `Left`); прежний
        // дефолт `Left` РОНЯЛ явный `<horizontalAlign>Left` на записи обоих форматов.
        FieldSpec::with_default(
            F_HORIZONTAL_ALIGN,
            "horizontalAlign",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        FieldSpec::with_default(
            F_VERTICAL_SCROLL,
            "verticalScroll",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        FieldSpec::with_default(
            F_CONVERSATIONS_REPRESENTATION,
            "conversationsRepresentation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        // Симметричные root-контейнер-атрибуты (оба формата эмитят не-дефолт; канон-дефолт =
        // общая омиссия — height `0`, align/spacing/width `Auto`; сверено по корпусу SSL).
        FieldSpec::with_default(F_HEIGHT, "height", ValueKind::Int, PropertyValue::Int(0)),
        FieldSpec::with_default(
            F_VERTICAL_ALIGN,
            "verticalAlign",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        FieldSpec::with_default(
            F_HORIZONTAL_SPACING,
            "horizontalSpacing",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        FieldSpec::with_default(
            F_CHILD_ITEMS_WIDTH,
            "childItemsWidth",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        FieldSpec::with_default(F_SCALE, "scale", ValueKind::Int, PropertyValue::Int(0)),
        FieldSpec::with_default(
            F_VERTICAL_SPACING,
            "verticalSpacing",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        // ERP-witnessed (M-волна 2) — опциональные (present ⟺ недефолт).
        FieldSpec::required(F_SCALING_MODE, "scalingMode", ValueKind::Enum),
        FieldSpec::required(
            F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT,
            "collapseItemsByImportanceVariant",
            ValueKind::Enum,
        ),
        FieldSpec::required(F_CHILDREN_ALIGN, "childrenAlign", ValueKind::Enum),
        FieldSpec::required(F_GROUP_LIST, "groupList", ValueKind::Str),
        FieldSpec::required(F_SETTINGS_STORAGE, "settingsStorage", ValueKind::Str),
    ]
}

/// Канонический [`EntitySpec`] форм-уровневых свойств (кэш на процесс).
pub fn form_root() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "FormRoot",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
