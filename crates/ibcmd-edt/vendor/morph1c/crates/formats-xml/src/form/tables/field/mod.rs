//! FormField: общее тело (`FORM_FIELD_COMMON`) + подмодули extInfo по семействам контролов.

use crate::form::fields::{fp, Codec, DesOmit, FieldProj, Policy, Region};
use crate::form::tables::{
    fpa, platform_default, F_FF_FOOTER_DATA_PATH, F_FF_FOOTER_PICTURE, F_FF_FOOTER_TEXT_COLOR,
    F_FF_TITLE_BACK_COLOR, F_FF_WIDTH_IN_CARD, KEEP_BOOL_TRUE,
};
use morph1c_core::spec::forms::controls::form_field as ff;

mod document_picture;
mod input;

pub(crate) use document_picture::*;
pub(crate) use input::*;

/// Общие свойства FormField (порядок = канонический спек `form_field_common`).
pub(crate) static FORM_FIELD_COMMON: &[FieldProj] = &[
    fpa(
        ff::F_DISPLAY_IMPORTANCE,
        "displayImportance",
        "DisplayImportance",
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_TITLE,
        "title",
        "Title",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_TITLE_TEXT_COLOR,
        "titleTextColor",
        "TitleTextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_VISIBLE,
        "visible",
        "Visible",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_ENABLED,
        "enabled",
        "Enabled",
        Region::Body,
        Codec::Bool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        ff::F_USER_VISIBLE,
        "userVisible",
        "UserVisible",
        Region::Body,
        Codec::CommonBool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        ff::F_DATA_PATH,
        "dataPath",
        "DataPath",
        Region::Body,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    fp(
        ff::F_DEFAULT_ITEM,
        "defaultItem",
        "DefaultItem",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_SKIP_ON_INPUT,
        "skipOnInput",
        "SkipOnInput",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // titleBackColor — Color Symmetric (метамодель Field: skipOnInput→titleBackColor→
    // titleLocation; ERP-witness Мастер_Направления `<TitleBackColor>#FCFCFC` после Title,
    // до EditMode; EDT-зеркала несут `<titleBackColor xsi:type="core:ColorRef">`).
    fp(
        F_FF_TITLE_BACK_COLOR,
        "titleBackColor",
        "TitleBackColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_TITLE_LOCATION,
        "titleLocation",
        "TitleLocation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_TITLE_HEIGHT,
        "titleHeight",
        "TitleHeight",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ff::F_SHORTCUT,
        "shortcut",
        "Shortcut",
        Region::Body,
        Codec::Text,
        Policy::Symmetric,
    ),
    fp(
        ff::F_TOOL_TIP,
        "toolTip",
        "ToolTip",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_TOOL_TIP_REPRESENTATION,
        "toolTipRepresentation",
        "ToolTipRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_READ_ONLY,
        "readOnly",
        "ReadOnly",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_HORIZONTAL_ALIGN,
        "horizontalAlign",
        "HorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_GROUP_HORIZONTAL_ALIGN,
        "groupHorizontalAlign",
        "GroupHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_VERTICAL_ALIGN,
        "verticalAlign",
        "VerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_GROUP_VERTICAL_ALIGN,
        "groupVerticalAlign",
        "GroupVerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_WARNING_ON_EDIT_REPRESENTATION,
        "warningOnEditRepresentation",
        "WarningOnEditRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_WARNING_ON_EDIT,
        "warningOnEdit",
        "WarningOnEdit",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_MARK_REQUIRED_COMPLETE,
        "markRequiredComplete",
        "MarkRequiredComplete",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EDIT_MODE,
        "editMode",
        "EditMode",
        Region::Body,
        Codec::EnumTok,
        Policy::EditMode,
    ),
    fp(
        ff::F_FIXING_IN_TABLE,
        "fixingInTable",
        "FixingInTable",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_CELL_HYPERLINK,
        "cellHyperlink",
        "CellHyperlink",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_AUTO_CELL_HEIGHT,
        "autoCellHeight",
        "AutoCellHeight",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_SHOW_IN_HEADER,
        "showInHeader",
        "ShowInHeader",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_HEADER_PICTURE,
        "headerPicture",
        "HeaderPicture",
        Region::Body,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        ff::F_HEADER_HORIZONTAL_ALIGN,
        "headerHorizontalAlign",
        "HeaderHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        // ПЛАТФОРМЕННЫЙ дефолт `Left` (Designer его опускает; EDT пишет ЯВНО) — в канон НЕ
        // кладётся, поэтому у контрола без свойства оба ридера дают ОДНО (отсутствие в bag).
        // EDT-омиссия по-прежнему значит ecore-дефолт `Auto` и хранится явно.
        platform_default(
            ff::HEADER_HORIZONTAL_ALIGN_EDT_DEFAULT,
            Some(ff::HEADER_HORIZONTAL_ALIGN_EDT_DEFAULT),
            ff::HEADER_HORIZONTAL_ALIGN_DEFAULT,
            DesOmit::Eq(ff::HEADER_HORIZONTAL_ALIGN_DEFAULT),
        ),
    ),
    fp(
        F_FF_WIDTH_IN_CARD,
        "widthInCard",
        "WidthInCard",
        Region::Body,
        Codec::EnumMap(&[("Half", "Half")]),
        Policy::Symmetric,
    ),
    fp(
        ff::F_SHOW_IN_FOOTER,
        "showInFooter",
        "ShowInFooter",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // footerDataPath — DataPath Symmetric (метамодель Field: showInFooter→footerDataPath→
    // footerText; ERP-witness ВариантыГрафиковКредитовИДепозитов «ОплатыСумма»: EDT
    // `<showInFooter>true</showInFooter><footerDataPath xsi:type="form:DataPath">` ⟷ Designer
    // `<EditMode/><FooterDataPath>СуммаОплаты</FooterDataPath>`).
    fp(
        F_FF_FOOTER_DATA_PATH,
        "footerDataPath",
        "FooterDataPath",
        Region::Body,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    // footerText — Localized Symmetric (метамодель FormField showInFooter→footerText; witness
    // СообщениеSMS ×2 колонки: EDT showInFooter→footerText→extInfo ⟷ Designer FooterText).
    fp(
        ff::F_FOOTER_TEXT,
        "footerText",
        "FooterText",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    // footerTextColor — Color Symmetric (метамодель Field: footerText→footerTextColor;
    // ERP-witness РабочееМестоМенеджераПоДоставке `<FooterTextColor>style:…` после EditMode,
    // до FooterFont/Width).
    fp(
        F_FF_FOOTER_TEXT_COLOR,
        "footerTextColor",
        "FooterTextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    // footerPicture — PictureRef Symmetric (метамодель футер-блока; ERP 2×; cf-отказ).
    fp(
        F_FF_FOOTER_PICTURE,
        "footerPicture",
        "FooterPicture",
        Region::Body,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    // footerHorizontalAlign — ДО autoWidthInTable (метамодель: footer-блок →
    // footerHorizontalAlign → … → autoWidthInTable → cellHyperlink*; SSL:
    // footerHorizontalAlign<autoWidthInTable×2, autoWidthInTable<cellHyperlink*×1 —
    // прежняя позиция после cellHyperlink* вскрыта ЖурналРегистрации.ОтборЖурналаРегистрации).
    fp(
        ff::F_FOOTER_HORIZONTAL_ALIGN,
        "footerHorizontalAlign",
        "FooterHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_AUTO_WIDTH_IN_TABLE,
        "autoWidthInTable",
        "AutoWidthInTable",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_CELL_HYPERLINK_REPRESENTATION,
        "cellHyperlinkRepresentation",
        "CellHyperlinkRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_CELL_HYPERLINK_DISPLAY_VARIANT,
        "cellHyperlinkDisplayVariant",
        "CellHyperlinkDisplayVariant",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_SHOW_TITLE_IN_CARD,
        "showTitleInCard",
        "ShowTitleInCard",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
];

/// Число головных полей общего тела FormField, эмитимых EDT ДО `<handlers>`/`extendedTooltip`/
/// `contextMenu`/`<type>` (по F_TOOL_TIP_REPRESENTATION включительно; +1 = `shortcut`);
/// остальные — после.
pub(crate) const FORM_FIELD_EDT_HEAD: usize = 14;
