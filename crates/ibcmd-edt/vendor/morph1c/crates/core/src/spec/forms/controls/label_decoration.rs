//! Канонический спек контрола `LabelDecoration` (надпись-декорация) — ARCHITECTURE.md
//! §1.4/§1.6, под-IR L1f.
//!
//! Это ПЕРВЫЙ контрол foundation-среза форм (пилот `CommonForm.ФормаПроизвольногоСообщения`
//! несёт ровно один такой контрол). EDT кодирует его как `<items xsi:type="form:Decoration">`
//! с дискриминатором `<type>Label</type>`; Designer — как элемент `<LabelDecoration>`.
//! Канонический вид один и тот же в обоих форматах (kind = LabelDecoration).
//!
//! # Поля (по пилоту; ПЕР-ФОРМАТНЫЕ дефолты в проекциях, §1.6)
//! Общие свойства (тело элемента), канонический порядок:
//! * `title` (Localized) — EDT `<title>` пары `<key>/<value>`, Designer `<Title
//!   formatted="false">` с `<v8:item>`; оба эмитят;
//! * `visible`/`enabled`/`userVisible` (Bool) — EDT эмитит, Designer опускает
//!   (Designer-дефолт true); `userVisible` = значение вложенного `<common>`;
//! * `maxWidth` (Int) — EDT `<maxWidth>`/Designer `<MaxWidth>`; оба эмитят;
//! * `autoMaxWidth`/`autoMaxHeight` (Bool) — EDT опускает (EDT-дефолт false), Designer
//!   эмитит `false` (Designer-дефолт true).
//!
//! extInfo (`form:LabelDecorationExtInfo`): `horizontalAlign` (Enum) — EDT
//! `<extInfo><horizontalAlign>`; Designer опускает (дефолт Left).
//!
//! Под-стабы `extendedTooltip`/`contextMenu` + событие `URLProcessing` — decorator-frame;
//! их моделирует КОННЕКТОР. ОБА формата несут СОДЕРЖАНИЕ тела (заголовок/`maxWidth`/`autoMax*`/
//! `horizontalStretch`/события) — оно X-сравнимо; лишь EDT-каркас (extInfo-обёртка) форматно-
//! асимметричен и нормализуется ДО X.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::spec::forms::controls::ControlSpec;

/// `title` — локализованный заголовок надписи (оба формата эмитят).
pub const F_TITLE: FieldId = FieldId(1);
/// `visible` — видимость (EDT-only emit; Designer-дефолт true).
pub const F_VISIBLE: FieldId = FieldId(2);
/// `enabled` — доступность (EDT-only emit; Designer-дефолт true).
pub const F_ENABLED: FieldId = FieldId(3);
/// `userVisible` — пользовательская видимость `<common>` (EDT-only emit; Designer-дефолт true).
pub const F_USER_VISIBLE: FieldId = FieldId(4);
/// `maxWidth` — максимальная ширина (символы); оба эмитят.
pub const F_MAX_WIDTH: FieldId = FieldId(5);
/// `autoMaxWidth` — авто-максимум ширины (EDT-дефолт false, Designer-дефолт true).
pub const F_AUTO_MAX_WIDTH: FieldId = FieldId(6);
/// `autoMaxHeight` — авто-максимум высоты (EDT-дефолт false, Designer-дефолт true).
pub const F_AUTO_MAX_HEIGHT: FieldId = FieldId(7);
/// `maxHeight` — макс. высота декорации (Int, симметрично; метамодель после `autoMaxHeight`,
/// до `horizontalStretch`). Witness — LabelDecoration НастройкиПользователей / PictureDecoration.
pub const F_MAX_HEIGHT: FieldId = FieldId(116);
/// `horizontalStretch` — горизонтальное растяжение (Bool). Встречается в теле
/// расширенной подсказки; дефолт false.
pub const F_HORIZONTAL_STRETCH: FieldId = FieldId(8);
/// `displayImportance` — важность отображения (EDT элемент / Designer атрибут).
pub const F_DISPLAY_IMPORTANCE: FieldId = FieldId(9);
/// `toolTip` — локализованная подсказка (оба формата, симметрично).
pub const F_TOOL_TIP: FieldId = FieldId(10);
/// `toolTipRepresentation` — представление подсказки (Enum, симметрично).
pub const F_TOOL_TIP_REPRESENTATION: FieldId = FieldId(11);
/// `formatted` — HTML-заголовок (EDT `<formatted>true` элемент; Designer — атрибут
/// `formatted` на `<Title>`; каркас-glue, не таблица).
pub const F_FORMATTED: FieldId = FieldId(12);
/// `width` — ширина (Int, симметрично).
pub const F_WIDTH: FieldId = FieldId(13);
/// `height` — высота (Int, симметрично).
pub const F_HEIGHT: FieldId = FieldId(14);
/// `groupHorizontalAlign` (Enum, симметрично).
pub const F_GROUP_HORIZONTAL_ALIGN: FieldId = FieldId(15);
/// `skipOnInput` — пропускать при вводе (Bool, симметрично; явные true/false).
pub const F_SKIP_ON_INPUT: FieldId = FieldId(16);
/// `groupVerticalAlign` (Enum, симметрично).
pub const F_GROUP_VERTICAL_ALIGN: FieldId = FieldId(17);
/// `textColor` — цвет текста (Color-кодек, симметрично).
pub const F_TEXT_COLOR: FieldId = FieldId(18);
/// `verticalStretch` — вертикальное растяжение (Bool, симметрично).
pub const F_VERTICAL_STRETCH: FieldId = FieldId(19);

/// extInfo: `horizontalAlign` — горизонтальное выравнивание текста надписи.
pub const F_EXT_HORIZONTAL_ALIGN: FieldId = FieldId(101);
/// extInfo: `hyperlink` — надпись-гиперссылка (Bool, симметрично).
pub const F_EXT_HYPERLINK: FieldId = FieldId(102);
/// extInfo: `backColor` — цвет фона (Color-кодек, симметрично).
pub const F_EXT_BACK_COLOR: FieldId = FieldId(103);
/// extInfo: `verticalAlign` — вертикальное выравнивание текста (Enum, симметрично).
pub const F_EXT_VERTICAL_ALIGN: FieldId = FieldId(104);
/// extInfo: `border` — рамка надписи (композит `core:BorderDef`; канон — стиль-Enum, ширина
/// всегда 1). EDT/Designer опускают дефолт `WithoutBorder`; ненулевой стиль (напр. `Overline`)
/// эмитят оба. Тот же [`crate::ir::value`]-Enum и кодек, что у PictureField/CalendarField.
pub const F_EXT_BORDER: FieldId = FieldId(109);
/// extInfo PictureDecoration: `picture` (Picture-кодек, оба эмитят).
pub const F_EXT_PICTURE: FieldId = FieldId(105);
/// extInfo PictureDecoration: `pictureSize` (Enum, симметрично).
pub const F_EXT_PICTURE_SIZE: FieldId = FieldId(106);
/// extInfo PictureDecoration: `fileDragMode` (KEEP: EDT опускает `AsFile`, Designer —
/// `AsFileRef`; cross-omission 87/13).
pub const F_EXT_FILE_DRAG_MODE: FieldId = FieldId(107);
/// extInfo PictureDecoration: `imageScale` (Int, симметрично).
pub const F_EXT_IMAGE_SCALE: FieldId = FieldId(108);
/// extInfo PictureDecoration: `zoomable` — масштабируемость картинки (Bool, симметрично:
/// оба формата эмитят `true`, оба опускают дефолт false; сверено корпусом покрытия). EDT-
/// позиция — перед `fileDragMode`; Designer — сразу после `<Title>`.
pub const F_EXT_ZOOMABLE: FieldId = FieldId(110);
/// extInfo PictureDecoration: `enableStartDrag` (Bool, симметрично: оба эмитят `true`,
/// оба опускают дефолт false). EDT-позиция — перед `fileDragMode`.
pub const F_EXT_ENABLE_START_DRAG: FieldId = FieldId(111);
/// extInfo PictureDecoration: `enableDrag` (Bool, симметрично). EDT-позиция — перед `fileDragMode`.
pub const F_EXT_ENABLE_DRAG: FieldId = FieldId(112);
/// extInfo LabelDecoration: `titleHeight` — высота заголовка (Int, симметрично; метамодель
/// `LabelDecorationExtInfo` после `verticalAlign`, до `backColor`).
pub const F_EXT_TITLE_HEIGHT: FieldId = FieldId(113);
/// extInfo PictureDecoration: `pictureColor` — цвет картинки (Color-Ref, симметрично; метамодель
/// `PictureDecorationExtInfo` после `pictureSize`/`backgroundShowMode`, до `zoomable`).
pub const F_EXT_PICTURE_COLOR: FieldId = FieldId(114);
/// extInfo PictureDecoration: `border` — рамка (Codec::Border; метамодель после `enableDrag`,
/// до `fileDragMode`).
pub const F_EXT_PIC_BORDER: FieldId = FieldId(115);
/// extInfo PictureDecoration: `nonselectedPictureText` — текст невыбранной картинки
/// (Localized, Symmetric). Метамодель PictureDecorationExtInfo: imageScale →
/// **nonselectedPictureText** → enableStartDrag. Witness — УправлениеПодключениемDSS
/// (EDT picture→nonselectedPictureText→fileDragMode ⟷ Designer
/// GroupHorizontalAlign→NonselectedPictureText→Picture).
/// БАГФИКС: прежний id 116 КОЛЛАЙДИЛ с [`F_MAX_HEIGHT`] (116) — Designer-слот
/// F(116) резолвился в maxHeight-строку ТЕЛА: NonselectedPictureText РОНЯЛСЯ на write
/// (DSS.ПодтверждениеОперацииМобильноеПриложение), а maxHeight ДУБЛИРОВАЛСЯ
/// (УничтожениеПерсональныхДанных.ФормаСозданияАктов).
pub const F_EXT_NONSELECTED_PICTURE_TEXT: FieldId = FieldId(117);

/// Канонический литерал `horizontalAlign = Left` (Designer-дефолт — Designer опускает `Left`).
pub const HORIZONTAL_ALIGN_LEFT: &str = "Left";
/// EDT-дефолт `horizontalAlign` LabelDecoration (EDT опускает `Auto`; сверено корпусом
/// покрытия — надпись `ГоризонтальноеПоложение_Авто` несёт ПУСТОЙ EDT extInfo, а `Left`/
/// `Center`/`Right` эмитятся). EDT и Designer имеют ПРОТИВОПОЛОЖНЫЕ дефолты (Auto ⟺ Left).
pub const HORIZONTAL_ALIGN_EDT_DEFAULT: &str = "Auto";
/// EDT-дефолт `fileDragMode` PictureDecoration (омиссия `AsFile`).
pub const FILE_DRAG_MODE_EDT_DEFAULT: &str = "AsFile";
/// Designer-дефолт `fileDragMode` PictureDecoration (омиссия `AsFileRef`).
pub const FILE_DRAG_MODE_DESIGNER_DEFAULT: &str = "AsFileRef";

fn sym(id: FieldId, name: &'static str, kind: ValueKind) -> FieldSpec {
    FieldSpec::required(id, name, kind)
}

/// ОБЩЕЕ тело декораций (`LabelDecoration` + `PictureDecoration`) в каноническом порядке
/// эмиссии (= порядок EDT-тела, topo 157+100 декораций, конфликтов 0): displayImportance,
/// title, visible, enabled, userVisible, toolTip, toolTipRepresentation, [decorator-стабы —
/// каркас], formatted, [type], maxWidth, width, autoMaxWidth, height, autoMaxHeight,
/// horizontalStretch, groupHorizontalAlign, skipOnInput, groupVerticalAlign, textColor,
/// verticalStretch. `font` (9 надписей) НЕ смоделирован — §1.0-громко.
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
        FieldSpec::with_default(
            F_VISIBLE,
            "visible",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_ENABLED,
            "enabled",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_USER_VISIBLE,
            "userVisible",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        sym(F_TOOL_TIP, "toolTip", ValueKind::Localized),
        sym(
            F_TOOL_TIP_REPRESENTATION,
            "toolTipRepresentation",
            ValueKind::Enum,
        ),
        sym(F_FORMATTED, "formatted", ValueKind::Bool),
        FieldSpec::with_default(
            F_MAX_WIDTH,
            "maxWidth",
            ValueKind::Int,
            PropertyValue::Int(0),
        ),
        sym(F_WIDTH, "width", ValueKind::Int),
        FieldSpec::with_default(
            F_AUTO_MAX_WIDTH,
            "autoMaxWidth",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        sym(F_HEIGHT, "height", ValueKind::Int),
        FieldSpec::with_default(
            F_AUTO_MAX_HEIGHT,
            "autoMaxHeight",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        sym(F_MAX_HEIGHT, "maxHeight", ValueKind::Int),
        sym(F_HORIZONTAL_STRETCH, "horizontalStretch", ValueKind::Bool),
        sym(
            F_GROUP_HORIZONTAL_ALIGN,
            "groupHorizontalAlign",
            ValueKind::Enum,
        ),
        sym(F_SKIP_ON_INPUT, "skipOnInput", ValueKind::Bool),
        sym(
            F_GROUP_VERTICAL_ALIGN,
            "groupVerticalAlign",
            ValueKind::Enum,
        ),
        sym(F_TEXT_COLOR, "textColor", ValueKind::Ref),
        sym(F_VERTICAL_STRETCH, "verticalStretch", ValueKind::Bool),
    ]
}

/// extInfo LabelDecoration в каноническом порядке = МЕТАМОДЕЛИ `LabelDecorationExtInfo`
/// (handlers-каркас, hyperlink, horizontalAlign, verticalAlign, titleHeight, backColor,
/// border) — сверено SSL: horizontalAlign<verticalAlign×147, verticalAlign<border×2;
/// прежняя позиция verticalAlign ПОСЛЕ border была слепой.
fn build_ext_info_fields() -> Vec<FieldSpec> {
    vec![
        sym(F_EXT_HYPERLINK, "hyperlink", ValueKind::Bool),
        FieldSpec::with_default(
            F_EXT_HORIZONTAL_ALIGN,
            "horizontalAlign",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(HORIZONTAL_ALIGN_LEFT)),
        ),
        sym(F_EXT_VERTICAL_ALIGN, "verticalAlign", ValueKind::Enum),
        sym(F_EXT_TITLE_HEIGHT, "titleHeight", ValueKind::Int),
        sym(F_EXT_BACK_COLOR, "backColor", ValueKind::Ref),
        sym(F_EXT_BORDER, "border", ValueKind::Enum),
    ]
}

/// extInfo PictureDecoration в каноническом порядке (= EDT extInfo: handlers-каркас,
/// picture, hyperlink, pictureSize, fileDragMode, imageScale).
fn build_picture_ext_fields() -> Vec<FieldSpec> {
    vec![
        sym(F_EXT_PICTURE, "picture", ValueKind::Ref),
        sym(F_EXT_HYPERLINK, "hyperlink", ValueKind::Bool),
        sym(F_EXT_PICTURE_SIZE, "pictureSize", ValueKind::Enum),
        sym(F_EXT_PICTURE_COLOR, "pictureColor", ValueKind::Ref),
        sym(F_EXT_ZOOMABLE, "zoomable", ValueKind::Bool),
        sym(F_EXT_ENABLE_START_DRAG, "enableStartDrag", ValueKind::Bool),
        sym(F_EXT_ENABLE_DRAG, "enableDrag", ValueKind::Bool),
        sym(F_EXT_PIC_BORDER, "border", ValueKind::Enum),
        sym(F_EXT_FILE_DRAG_MODE, "fileDragMode", ValueKind::Enum),
        sym(F_EXT_IMAGE_SCALE, "imageScale", ValueKind::Int),
        FieldSpec::with_default(
            F_EXT_NONSELECTED_PICTURE_TEXT,
            "nonselectedPictureText",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
    ]
}

/// Канонический спек ОБЩЕГО тела декораций (кэш на процесс; общий Label/Picture).
pub fn decoration_body() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Decoration",
        fields: Box::leak(build_property_fields().into_boxed_slice()),
        children: &[],
    })
}

/// Канонический спек extInfo-свойств `LabelDecoration` (кэш на процесс).
fn ext_info_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "LabelDecorationExtInfo",
        fields: Box::leak(build_ext_info_fields().into_boxed_slice()),
        children: &[],
    })
}

/// Канонический спек extInfo-свойств `PictureDecoration` (кэш на процесс).
fn picture_ext_info_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "PictureDecorationExtInfo",
        fields: Box::leak(build_picture_ext_fields().into_boxed_slice()),
        children: &[],
    })
}

/// Канонический [`ControlSpec`] вида `LabelDecoration` (кэш на процесс).
pub fn label_decoration() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "LabelDecoration",
        properties: decoration_body(),
        ext_info: ext_info_spec(),
        container: false, // надпись — лист, без рекурсивных детей.
    })
}

/// Канонический [`ControlSpec`] вида `PictureDecoration` (лист; EDT — `form:Decoration`
/// БЕЗ `<type>`, Designer — элемент `<PictureDecoration>`).
pub fn picture_decoration() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "PictureDecoration",
        properties: decoration_body(),
        ext_info: picture_ext_info_spec(),
        container: false,
    })
}
