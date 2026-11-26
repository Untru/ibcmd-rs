//! Канонические спеки КОНТРОЛОВ форм (ARCHITECTURE.md §5, под-IR L1f §1.2).
//!
//! Модуль-на-контрол: `label_decoration.rs`, `input_field.rs`, … — каждый отдаёт
//! [`ControlSpec`] своего вида. Зеркалится в `formats/*/src/forms/controls/<контрол>.rs`
//! (проекция: имена тегов EDT/Designer).
//!
//! # Что такое [`ControlSpec`]
//! Контрол моделируется ДВУМЯ [`EntitySpec`]'ами:
//! * `properties` — ОБЩИЕ свойства контрола (видимость/размеры/title/dataPath…), лежащие
//!   в «теле» элемента контрола;
//! * `ext_info` — ТИП-СПЕЦИФИЧНЫЕ свойства (`<extInfo xsi:type="form:<Type>ExtInfo">`),
//!   которых у разных видов разный набор.
//!
//! Плюс дискриминатор-метаданные: канонический `kind` (вид) и `container` (несёт ли
//! рекурсивных детей). Канонику (id/value_kind/порядок/дефолты/нормализацию) держат эти
//! `EntitySpec`'и — формат лишь проецирует (§1.6), как у metadata-видов.
//!
//! Это форм-внутренний аналог child-objects-субстрата: контрол-дерево рекурсивно (§3.4),
//! но узлы — [`crate::ir::FormItem`] (а не `MetadataObject`), и рекурсия идёт по `items`/
//! `ChildItems`, а не по metadata-`ChildObjects`. Движок рекурсии — в `formats-xml`
//! (`form_tree.rs`); `core` несёт лишь ДАННЫЕ (спеки).

use crate::spec::common::EntitySpec;

pub mod button;
pub mod form_field;
pub mod form_group;
pub mod label_decoration;
pub mod radio_button;
pub mod table;

/// Каноническая спецификация ОДНОГО вида контрола формы.
///
/// Два `EntitySpec`'а (общие свойства + extInfo) + дискриминатор-метаданные. `container`
/// сообщает рекурсии, навигировать ли в детей `items`/`ChildItems` этого узла (FormGroup/
/// Pages/Page/Popup/Table — `true`; листья — `false`).
#[derive(Debug, Clone, Copy)]
pub struct ControlSpec {
    /// Канонический код вида контрола (`"LabelDecoration"`, `"InputField"`, …) — ТОТ ЖЕ,
    /// что [`crate::ir::FormControlKind`].
    pub kind: &'static str,
    /// Спек ОБЩИХ свойств контрола (тело элемента). Поля в каноническом порядке эмиссии.
    pub properties: &'static EntitySpec,
    /// Спек ТИП-СПЕЦИФИЧНЫХ свойств (`extInfo`). Пустой спек (`fields: &[]`) ⇒ вид без
    /// extInfo-региона.
    pub ext_info: &'static EntitySpec,
    /// Несёт ли контрол рекурсивных детей (`items`/`ChildItems`). Лист ⇒ `false`.
    pub container: bool,
}

impl ControlSpec {
    /// Есть ли у вида непустой extInfo-регион (хоть одно extInfo-поле).
    pub fn has_ext_info(&self) -> bool {
        !self.ext_info.fields.is_empty()
    }
}

/// СВЁРНУТАЯ ПАРА канона: одно каноническое поле, у которого дамп платформы кодирует ЧАСТЬ
/// значений ДВУМЯ тегами.
///
/// # Зачем это здесь
/// Версионные факты сняты с ТЕГОВ дампа (`models/version_facts.jsonl`), а канон хранит поле.
/// Там, где отображение одно-к-одному, перевод тривиален (регистр первой буквы). Там, где
/// канон СВЕРНУЛ пару тегов в один литерал, перевод знает только МОДЕЛЬ — вот он, здесь.
/// Версий в этой таблице нет: она говорит лишь, ЧТО чем кодируется; КОГДА появился
/// компаньон — отвечают данные (`version_facts`).
///
/// Witnessed: Designer кодирует `editMode = Auto` парой `<EditMode>EnterOnInput` +
/// `<AutoEditMode>true`, а `editMode = EnterOnInput` — тем же `<EditMode>EnterOnInput` БЕЗ
/// компаньона (SSL 8.5.1: 4 313 пар против 484 одиночек — оба состояния реальны и различимы).
/// В дампах 2.17/2.20 компаньона нет ВООБЩЕ, поэтому там `EnterOnInput` неотличим от `Auto`;
/// поднимая такой дамп, платформа разрешает его в `Auto` (86 из 86 узлов витнесса).
#[derive(Debug, Clone, Copy)]
pub struct FoldedLiteral {
    /// Каноническое имя ПОЛЯ, чей литерал свёрнут (`"editMode"`).
    pub field: &'static str,
    /// Каноническое имя КОМПАНЬОНА — тега-флага, которого в каноне нет отдельным полем
    /// (`"autoEditMode"`). Ключ к версионным фактам.
    pub companion: &'static str,
    /// Литерал, которым выглядит поле, когда компаньон НЕ выставлен (`"EnterOnInput"`).
    pub without_companion: &'static str,
    /// Литерал, которым выглядит поле, когда компаньон выставлен (`"Auto"`).
    pub with_companion: &'static str,
}

/// Все свёрнутые пары канона форм (см. [`FoldedLiteral`]).
pub static FOLDED_LITERALS: &[FoldedLiteral] = &[FoldedLiteral {
    field: "editMode",
    companion: "autoEditMode",
    without_companion: form_field::EDIT_MODE_AUTO_RESOLVED,
    with_companion: form_field::EDIT_MODE_AUTO,
}];

/// ЛИТЕРАЛ МОЛЧАНИЯ: значение, которым ридер ВОССТАНАВЛИВАЕТ пропущенный тег Designer-дампа.
///
/// # Зачем это правилу 2
/// Правило 2 обязано отличать «источник ЗАДАЛ значение» от «источник промолчал». В мешке эти
/// два состояния для keep-полей СЛИЛИСЬ: ридер восстанавливает омиссию константой диалекта
/// (`Policy::Keep`, `des_fill`), и в мешке она неотличима от явно заданного того же литерала.
/// Различить помогает сам Designer: он ОПУСКАЕТ ровно этот литерал (`DesOmit::Eq(des_fill)`),
/// то есть явно задать его в дампе НЕВОЗМОЖНО — значение в мешке, равное литералу молчания,
/// всегда пришло из омиссии.
///
/// Значение здесь — ТА ЖЕ константа спека, которую формат берёт себе `des_fill`'ом
/// (`fg::DESIGNER_AUTO`), а не её копия: второго источника правды не заводится.
///
/// Таблица нужна ровно там, где есть факт `materialized`: у остальных полей молчание значит
/// одно и то же во всех версиях, и переписывать нечего.
#[derive(Debug, Clone, Copy)]
pub struct SilentLiteral {
    /// Вид контрола-владельца.
    pub owner: &'static str,
    /// Каноническое имя поля.
    pub field: &'static str,
    /// Литерал, которым ридер восстанавливает омиссию тега.
    pub literal: &'static str,
}

/// Литералы молчания (см. [`SilentLiteral`]).
///
/// Witnessed: `<Group>` у `UsualGroup`/`Page` Designer опускает во ВСЕХ версиях, но означает
/// омиссия РАЗНОЕ — 8.3.24/8.3.27 читают её как `Auto` (наши байты `.cf` совпадают с
/// платформенными на диагонали 2.17→2.17 и 2.20→2.20), а 8.5.1, поднимая тот же дамп, кладёт
/// `HorizontalIfPossible` (UsualGroup) / `Vertical` (Page) — факт `materialized`.
pub static SILENT_LITERALS: &[SilentLiteral] = &[
    SilentLiteral {
        owner: "UsualGroup",
        field: "group",
        literal: form_group::DESIGNER_AUTO,
    },
    SilentLiteral {
        owner: "Page",
        field: "group",
        literal: form_group::DESIGNER_AUTO,
    },
];

/// Литерал молчания поля `owner`.`field`, если он объявлен.
pub fn silent_literal(owner: &str, field: &str) -> Option<&'static str> {
    SILENT_LITERALS
        .iter()
        .find(|s| s.owner == owner && s.field == field)
        .map(|s| s.literal)
}

/// Регистрация вида контрола в реестре (символ-держатель для DCE-safe codegen, как
/// `SpecRegistration` метаданных). Реестр контролов строится из файловой раскладки.
#[derive(Debug, Clone, Copy)]
pub struct FormControlRegistration {
    /// Канонический код вида контрола.
    pub kind: &'static str,
    /// Конструктор его [`ControlSpec`].
    pub ctor: fn() -> &'static ControlSpec,
}

/// Лук-ап [`ControlSpec`] по каноническому коду вида контрола. Foundation несёт лишь
/// зарегистрированные виды (срезы добавляют новые модуль-на-контрол).
pub fn control_spec_for(kind: &str) -> Option<&'static ControlSpec> {
    REGISTRY.iter().find(|r| r.kind == kind).map(|r| (r.ctor)())
}

/// Реестр видов контролов. Растёт по мере добавления модулей-на-контрол; держится явным
/// массивом (DCE-safe, как metadata `SPEC_CTORS`).
static REGISTRY: &[FormControlRegistration] = &[
    FormControlRegistration {
        kind: "LabelDecoration",
        ctor: label_decoration::label_decoration,
    },
    FormControlRegistration {
        kind: "PictureDecoration",
        ctor: label_decoration::picture_decoration,
    },
    FormControlRegistration {
        kind: "UsualGroup",
        ctor: form_group::usual_group,
    },
    FormControlRegistration {
        kind: "Pages",
        ctor: form_group::pages,
    },
    FormControlRegistration {
        kind: "Page",
        ctor: form_group::page,
    },
    FormControlRegistration {
        kind: "ButtonGroup",
        ctor: form_group::button_group,
    },
    FormControlRegistration {
        kind: "CommandBar",
        ctor: form_group::command_bar,
    },
    FormControlRegistration {
        kind: "Popup",
        ctor: form_group::popup,
    },
    FormControlRegistration {
        kind: "Button",
        ctor: button::button,
    },
    FormControlRegistration {
        kind: "InputField",
        ctor: form_field::input_field,
    },
    FormControlRegistration {
        kind: "CheckBoxField",
        ctor: form_field::check_box_field,
    },
    FormControlRegistration {
        kind: "LabelField",
        ctor: form_field::label_field,
    },
    FormControlRegistration {
        kind: "HTMLDocumentField",
        ctor: form_field::html_document_field,
    },
    FormControlRegistration {
        kind: "ProgressBarField",
        ctor: form_field::progress_bar_field,
    },
    FormControlRegistration {
        kind: "FormattedDocumentField",
        ctor: form_field::formatted_document_field,
    },
    FormControlRegistration {
        kind: "TextDocumentField",
        ctor: form_field::text_document_field,
    },
    FormControlRegistration {
        kind: "PictureField",
        ctor: form_field::picture_field,
    },
    FormControlRegistration {
        kind: "SpreadsheetDocumentField",
        ctor: form_field::spreadsheet_document_field,
    },
    FormControlRegistration {
        kind: "CalendarField",
        ctor: form_field::calendar_field,
    },
    FormControlRegistration {
        kind: "TrackBarField",
        ctor: form_field::track_bar_field,
    },
    FormControlRegistration {
        kind: "PeriodField",
        ctor: form_field::period_field,
    },
    FormControlRegistration {
        kind: "GraphicalSchemaField",
        ctor: form_field::graphical_schema_field,
    },
    FormControlRegistration {
        kind: "ChartField",
        ctor: form_field::chart_field,
    },
    FormControlRegistration {
        kind: "GanttChartField",
        ctor: form_field::gantt_chart_field,
    },
    FormControlRegistration {
        kind: "PDFDocumentField",
        ctor: form_field::pdf_document_field,
    },
    FormControlRegistration {
        kind: "RadioButtonField",
        ctor: radio_button::radio_button_field,
    },
    FormControlRegistration {
        kind: "Table",
        ctor: table::table,
    },
    FormControlRegistration {
        kind: "ColumnGroup",
        ctor: form_group::column_group,
    },
];

/// Все зарегистрированные виды контролов (по коду).
pub fn registered_controls() -> Vec<&'static str> {
    REGISTRY.iter().map(|r| r.kind).collect()
}
