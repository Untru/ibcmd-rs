//! FormGroup: общее тело + extInfo (UsualGroup/Pages/Page/ButtonGroup/CommandBar/ColumnGroup/Popup).

use crate::form::fields::{Codec, DesOmit, FieldProj, Keep, Policy, Region, fp};
use crate::form::tables::{
    F_GRP_ASSOCIATED_TABLE, F_GRP_HIDDEN_STATE_TITLE_BACK_COLOR, F_GRP_POPUP_BORDER_COLOR,
    F_GRP_TITLE_BACK_COLOR, KEEP_BOOL_TRUE, fpa, keep, xml220,
};
use morph1c_core::spec::forms::controls::form_field as ff;
use morph1c_core::spec::forms::controls::form_group as fg;

/// Мэппинг литералов `childrenAlign` (UsualGroup/Page extInfo), `(канон=EDT, Designer)`.
///
/// ПЯТЬ литералов диалекты пишут ДОСЛОВНО ОДИНАКОВО, ШЕСТОЙ — АЛИАС ИМЕНИ: EDT
/// `ItemsAutoTitlesLeft` ⟺ Designer `TitlesLeftDataAuto` (одно и то же значение, ординал `6`
/// у ячейки cf — см. `CHILDREN_ALIGN` в `formats-cf/src/form_body/group/mod.rs`).
///
/// ЦЕНЗ (покарьерный join диалектов по (форма, `id` носителя, тег) — ПОЛНЫЙ корпус
/// ERP 8.3.27 11 400 форм ⟂ SSL 8.5.1 876 форм ⟂ coverage s10_forms):
///   ERP: 63 носителя в КАЖДОМ диалекте; расходится РОВНО 3 (`TitlesLeftDataAuto` ⟂
///   `ItemsAutoTitlesLeft`) — `CommonForms/НастройкаПодтвержденияКриптоопераций` UsualGroup id=151,
///   `CommonForms/ОтключениеПодтвержденияКриптоопераций` UsualGroup id=107,
///   `InformationRegisters/УчетныеЗаписиЭДО/Forms/ПомощникПодключенияЭДО` Page id=905;
///   остальные 60 — дословно равны. SSL: множества литералов у диалектов СОВПАДАЮТ (шестого
///   литерала в корпусе нет). coverage `Форма_Группы`/`Форма_ГруппыСпец`: по 5 общих
///   литералов на форму, шестого нет.
///
/// ⚠ Домен ЗАКРЫТ (Codec::EnumMap отказывает §1.0 на невитнессированном литерале). Литерал
/// `Auto` метамодели EDT (`FormChildrenAlign`) в корпусе НЕ ВСТРЕЧАЕТСЯ ни в одном диалекте —
/// это ОМИССИЯ-дефолт (ячейка cf `0`), поэтому в таблицу он НЕ внесён. Симметрично,
/// `ItemsAutoTitlesLeft` в дампе метамодели ОТСУТСТВУЕТ (`enum_literals` перечисляют лишь
/// Auto/None/Items*×4) — корпус здесь СТАРШЕ метамодели, витнесс решает.
const CHILDREN_ALIGN_MAP: &[(&str, &str)] = &[
    ("None", "None"),
    ("ItemsLeftTitlesLeft", "ItemsLeftTitlesLeft"),
    ("ItemsRightTitlesLeft", "ItemsRightTitlesLeft"),
    ("ItemsLeftTitlesRight", "ItemsLeftTitlesRight"),
    ("ItemsRightTitlesRight", "ItemsRightTitlesRight"),
    // ЕДИНСТВЕННЫЙ алиас имени (ERP ×3).
    ("ItemsAutoTitlesLeft", "TitlesLeftDataAuto"),
];

// ============================ FormGroup: общее тело ============================

/// Общее тело семейства FormGroup (порядок = канонический спек `form_group_body`).
pub(crate) static FORM_GROUP_BODY: &[FieldProj] = &[
    fpa(
        fg::F_DISPLAY_IMPORTANCE,
        "displayImportance",
        "DisplayImportance",
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_VISIBLE,
        "visible",
        "Visible",
        Region::Body,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        fg::F_ENABLED,
        "enabled",
        "Enabled",
        Region::Body,
        Codec::Bool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        fg::F_USER_VISIBLE,
        "userVisible",
        "UserVisible",
        Region::Body,
        Codec::CommonBool,
        KEEP_BOOL_TRUE,
    ),
    fp(
        fg::F_TITLE,
        "title",
        "Title",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        fg::F_TITLE_TEXT_COLOR,
        "titleTextColor",
        "TitleTextColor",
        Region::Body,
        Codec::Color,
        Policy::Symmetric,
    ),
    // EDT-канон: `toolTip` ПЕРЕД `readOnly` (сверено корпусом — UsualGroup «Последнее
    // отправление» в `НастройкиСинхронизацииДанных`; тот же порядок, что у FormField). Designer
    // эмитит их в собственном порядке (`DES_GROUP_ORDER`, независим от этой таблицы).
    fp(
        fg::F_TOOL_TIP,
        "toolTip",
        "ToolTip",
        Region::Body,
        Codec::Localized,
        Policy::Symmetric,
    ),
    // toolTipRepresentation — СРАЗУ после toolTip (метамодель FormGroup toolTip→
    // toolTipRepresentation→readOnly; SSL: toolTip<TTR×27, TTR<width×16,
    // TTR<enableContentChange×1 — прежняя позиция после shortcut вскрыта РассылкиОтчетов).
    fp(
        fg::F_TOOL_TIP_REPRESENTATION,
        "toolTipRepresentation",
        "ToolTipRepresentation",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_READ_ONLY,
        "readOnly",
        "ReadOnly",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_ENABLE_CONTENT_CHANGE,
        "enableContentChange",
        "EnableContentChange",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_SHORTCUT,
        "shortcut",
        "Shortcut",
        Region::Body,
        Codec::Text,
        Policy::Symmetric,
    ),
    fp(
        fg::F_WIDTH,
        "width",
        "Width",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        fg::F_HEIGHT,
        "height",
        "Height",
        Region::Body,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        fg::F_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_VERTICAL_STRETCH,
        "verticalStretch",
        "VerticalStretch",
        Region::Body,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_GROUP_HORIZONTAL_ALIGN,
        "groupHorizontalAlign",
        "GroupHorizontalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_GROUP_VERTICAL_ALIGN,
        "groupVerticalAlign",
        "GroupVerticalAlign",
        Region::Body,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

// ============================ FormGroup: extInfo ============================

/// extInfo UsualGroup (KEEP-политики — cross-omission witnesses, см. спек).
pub(crate) static USUAL_GROUP_EXT: &[FieldProj] = &[
    xml220(
        fp(
            fg::F_EXT_GROUP,
            "group",
            "Group",
            Region::Ext,
            Codec::EnumTok,
            keep(
                fg::GROUP_EDT_DEFAULT,
                Some(fg::GROUP_EDT_DEFAULT),
                fg::DESIGNER_AUTO,
                DesOmit::Eq(fg::DESIGNER_AUTO),
            ),
        ),
        keep(
            fg::GROUP_EDT_DEFAULT,
            Some(fg::GROUP_EDT_DEFAULT),
            "HorizontalIfPossible",
            DesOmit::Eq("HorizontalIfPossible"),
        ),
    ),
    fp(
        fg::F_EXT_CHILDREN_ALIGN,
        "childrenAlign",
        "ChildrenAlign",
        Region::Ext,
        Codec::EnumMap(CHILDREN_ALIGN_MAP),
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_HORIZONTAL_SPACING,
        "horizontalSpacing",
        "HorizontalSpacing",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // Порядок = МЕТАМОДЕЛЬ (verticalSpacing → horizontalAlign → verticalAlign): SSL
    // horizontalSpacing<verticalSpacing×12, verticalSpacing<horizontalAlign×12,
    // horizontalAlign<verticalAlign×6, 0 контрпримеров (прежний verticalAlign-первым —
    // tie-break-артефакт, вскрыт РассылкиОтчетов после снятия read-блокера).
    fp(
        fg::F_EXT_VERTICAL_SPACING,
        "verticalSpacing",
        "VerticalSpacing",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_HORIZONTAL_ALIGN,
        "horizontalAlign",
        "HorizontalAlign",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_VERTICAL_ALIGN,
        "verticalAlign",
        "VerticalAlign",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_BEHAVIOR,
        "behavior",
        "Behavior",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::BEHAVIOR_EDT_DEFAULT,
            Some(fg::BEHAVIOR_EDT_DEFAULT),
            fg::DESIGNER_AUTO,
            DesOmit::Eq(fg::DESIGNER_AUTO),
        ),
    ),
    fp(
        fg::F_EXT_COLLAPSED_REPRESENTATION_TITLE,
        "collapsedRepresentationTitle",
        "CollapsedRepresentationTitle",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_COLLAPSED,
        "collapsed",
        "Collapsed",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_CONTROL_REPRESENTATION,
        "controlRepresentation",
        "ControlRepresentation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SCROLL_ON_COMPRESS,
        "scrollOnCompress",
        "ScrollOnCompress",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    xml220(
        fp(
            fg::F_EXT_REPRESENTATION,
            "representation",
            "Representation",
            Region::Ext,
            Codec::EnumTok,
            keep(
                fg::REPRESENTATION_EDT_DEFAULT,
                Some(fg::REPRESENTATION_EDT_DEFAULT),
                fg::DESIGNER_AUTO,
                DesOmit::Eq(fg::DESIGNER_AUTO),
            ),
        ),
        keep(
            fg::REPRESENTATION_EDT_DEFAULT,
            Some(fg::REPRESENTATION_EDT_DEFAULT),
            "WeakSeparation",
            DesOmit::Eq("WeakSeparation"),
        ),
    ),
    fp(
        fg::F_EXT_HYPERLINK,
        "hyperlink",
        "Hyperlink",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SHOW_AS_CARD,
        "showAsCard",
        "ShowAsCard",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SHOW_LEFT_MARGIN,
        "showLeftMargin",
        "ShowLeftMargin",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        fg::F_EXT_UNITED,
        "united",
        "United",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    // slaveItemsWidth ПЕРЕД backColor/showTitle/throughAlign/currentRowUse (SSL UsualGroupExtInfo:
    // showLeftMargin/united/representation<slaveItemsWidth, slaveItemsWidth<currentRowUse×61/
    // throughAlign×60/showTitle×21/backColor×1). Designer-порядок (DES_GROUP_ORDER) уже верен.
    fp(
        fg::F_EXT_SLAVE_ITEMS_WIDTH,
        "slaveItemsWidth",
        "ChildItemsWidth",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // format — Localized Symmetric (метамодель UsualGroupExtInfo slaveItemsWidth→format→showTitle;
    // witness ЗагрузкаКурсовВалют ×2: EDT united→format→showTitle/throughAlign).
    fp(
        fg::F_EXT_FORMAT,
        "format",
        "Format",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    xml220(
        fp(
            fg::F_EXT_SHOW_TITLE,
            "showTitle",
            "ShowTitle",
            Region::Ext,
            Codec::EnumTok,
            keep(
                fg::SHOW_TITLE_EDT_DEFAULT,
                Some(fg::SHOW_TITLE_EDT_DEFAULT),
                fg::DESIGNER_AUTO_LOWER,
                DesOmit::Eq(fg::DESIGNER_AUTO_LOWER),
            ),
        ),
        keep(
            fg::SHOW_TITLE_EDT_DEFAULT,
            Some(fg::SHOW_TITLE_EDT_DEFAULT),
            "true",
            DesOmit::Eq("true"),
        ),
    ),
    fp(
        fg::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_TITLE_DATA_PATH,
        "titleDataPath",
        "TitleDataPath",
        Region::Ext,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_THROUGH_ALIGN,
        "throughAlign",
        "ThroughAlign",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::THROUGH_ALIGN_EDT_DEFAULT,
            Some(fg::THROUGH_ALIGN_EDT_DEFAULT),
            fg::DESIGNER_AUTO,
            DesOmit::Eq(fg::DESIGNER_AUTO),
        ),
    ),
    // currentRowUse: ПРОТИВОПОЛОЖНЫЕ дефолты (EDT опускает `Use`, Designer опускает `Auto`;
    // сверено корпусом покрытия — Designer эмитит `Use`/`DontUse`, EDT эмитит `Auto`/`DontUse`).
    fp(
        fg::F_EXT_CURRENT_ROW_USE,
        "currentRowUse",
        "CurrentRowUse",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::CURRENT_ROW_USE_EDT_DEFAULT,
            Some(fg::CURRENT_ROW_USE_EDT_DEFAULT),
            fg::DESIGNER_AUTO,
            DesOmit::Eq(fg::DESIGNER_AUTO),
        ),
    ),
    fp(
        F_GRP_ASSOCIATED_TABLE,
        "associatedTableElementId",
        "AssociatedTableElementId",
        Region::Ext,
        Codec::Value,
        Policy::Symmetric,
    ),
    // hiddenStateTitleBackColor — Color Symmetric (метамодель UsualGroupExtInfo#28;
    // ERP 1⟷1). cf: {38}-композит cells[23] (абляция s10 style:BorderColor).
    fp(
        F_GRP_HIDDEN_STATE_TITLE_BACK_COLOR,
        "hiddenStateTitleBackColor",
        "HiddenStateTitleBackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
];

/// extInfo Pages.
pub(crate) static PAGES_EXT: &[FieldProj] = &[
    fp(
        fg::F_EXT_PAGES_REPRESENTATION,
        "pagesRepresentation",
        "PagesRepresentation",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::PAGES_REPRESENTATION_EDT_DEFAULT,
            Some(fg::PAGES_REPRESENTATION_EDT_DEFAULT),
            fg::DESIGNER_AUTO,
            DesOmit::Eq(fg::DESIGNER_AUTO),
        ),
    ),
    // currentRowUse: ПРОТИВОПОЛОЖНЫЕ дефолты (как UsualGroup — EDT опускает `Use`, Designer `Auto`).
    fp(
        fg::F_EXT_CURRENT_ROW_USE,
        "currentRowUse",
        "CurrentRowUse",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::CURRENT_ROW_USE_EDT_DEFAULT,
            Some(fg::CURRENT_ROW_USE_EDT_DEFAULT),
            fg::DESIGNER_AUTO,
            DesOmit::Eq(fg::DESIGNER_AUTO),
        ),
    ),
    fp(
        F_GRP_ASSOCIATED_TABLE,
        "associatedTableElementId",
        "AssociatedTableElementId",
        Region::Ext,
        Codec::Value,
        Policy::Symmetric,
    ),
];

/// extInfo Page.
pub(crate) static PAGE_EXT: &[FieldProj] = &[
    fp(
        fg::F_EXT_PICTURE,
        "picture",
        "Picture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    // group — KEEP, как у UsualGroup. Прежний вердикт «EDT опускает у 2/78 страниц БЕЗ
    // разрешимого правила» (⇒ `Policy::DesKeep`, presence-точная EDT-сторона) ОПРОВЕРГНУТ
    // кросс-витнесс-цензом: он смотрел ТОЛЬКО EDT-дамп. Join по (форма, имя, id) со ВТОРЫМ
    // дампом той же конфигурации даёт ДИЗЪЮНКТНУЮ таблицу, клетки «оба опускают» НЕТ:
    //   SSL 898/898 страниц: des `<ABSENT>`⟺edt `Auto` 76 | des `Horizontal`⟺edt `<ABSENT>` 2 |
    //                        зеркальные Vertical 673 / HorizontalIfPossible 69 /
    //                        AlwaysHorizontal 43 / AutoScreenTypeSensitive 35;
    //   ERP 12 220/12 220:   des `<ABSENT>`⟺edt `Vertical` 10 371 |
    //                        des `Horizontal`⟺edt `<ABSENT>` 1 028 |
    //                        AlwaysHorizontal 591 / HorizontalIfPossible 230;
    //   coverage 36/36:      des `<ABSENT>`⟺edt `Auto` 36.
    // ⇒ EDT опускает РОВНО свой ecore-дефолт [`fg::GROUP_EDT_DEFAULT`] (`Horizontal`) — тот же,
    // что у UsualGroup, — а Designer в этих же клетках пишет `Horizontal` ЯВНО (правило ЕСТЬ).
    // `Policy::PlatformDefault` здесь НЕПРИМЕНИМ: он сделал бы канон-омиссией то, что опускает
    // Designer, а cf-писатель уже занял омиссию бага под `Horizontal` (ячейки Page ext
    // `[2]/[16]/[17]/[20]`: absent → `1,1,1,1` = РОВНО код `Horizontal`, тогда как `Auto` →
    // `1,1,0,4`) ⇒ 76 SSL-страниц сменили бы БАЙТЫ. KEEP оставляет обе омиссии пер-форматными
    // (R байт-точен с обеих сторон) и даёт ОБОИМ ридерам один канон.
    // ⚠ ОСТАТОК (версионный вход, класс `buttonImportance`): литерал, который ОПУСКАЕТ Designer,
    // ВЕРСИОНЕН — `Auto` в дампах 8.5.1 (SSL/coverage), `Vertical` в дампе 8.3.27 (ERP);
    // `des_fill` — константа, поэтому на ERP клетка 10 371 остаётся X-неравной.
    xml220(
        fp(
            fg::F_EXT_GROUP,
            "group",
            "Group",
            Region::Ext,
            Codec::EnumTok,
            keep(
                fg::GROUP_EDT_DEFAULT,
                Some(fg::GROUP_EDT_DEFAULT),
                fg::DESIGNER_AUTO,
                DesOmit::Eq(fg::DESIGNER_AUTO),
            ),
        ),
        keep(
            fg::GROUP_EDT_DEFAULT,
            Some(fg::GROUP_EDT_DEFAULT),
            "Vertical",
            DesOmit::Eq("Vertical"),
        ),
    ),
    fp(
        fg::F_EXT_CHILDREN_ALIGN,
        "childrenAlign",
        "ChildrenAlign",
        Region::Ext,
        Codec::EnumMap(CHILDREN_ALIGN_MAP),
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_HORIZONTAL_SPACING,
        "horizontalSpacing",
        "HorizontalSpacing",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // Порядок = МЕТАМОДЕЛЬ PageGroupExtInfo (verticalSpacing → horizontalAlign → verticalAlign →
    // slaveItemsWidth): SSL horizontalAlign<verticalAlign×2, group<slaveItemsWidth×5,
    // slaveItemsWidth<showTitle×4; 0 контрпримеров — прежний порядок (slaveItemsWidth-ранним,
    // verticalAlign до horizontalAlign) вскрыт ШаблоныАнкет/УправлениеПодключениемDSS.
    fp(
        fg::F_EXT_VERTICAL_SPACING,
        "verticalSpacing",
        "VerticalSpacing",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_HORIZONTAL_ALIGN,
        "horizontalAlign",
        "HorizontalAlign",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_VERTICAL_ALIGN,
        "verticalAlign",
        "VerticalAlign",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SLAVE_ITEMS_WIDTH,
        "slaveItemsWidth",
        "ChildItemsWidth",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    xml220(
        fp(
            fg::F_EXT_SHOW_TITLE,
            "showTitle",
            "ShowTitle",
            Region::Ext,
            Codec::EnumTok,
            keep(
                fg::SHOW_TITLE_EDT_DEFAULT,
                Some(fg::SHOW_TITLE_EDT_DEFAULT),
                fg::DESIGNER_AUTO_LOWER,
                DesOmit::Eq(fg::DESIGNER_AUTO_LOWER),
            ),
        ),
        keep(
            fg::SHOW_TITLE_EDT_DEFAULT,
            Some(fg::SHOW_TITLE_EDT_DEFAULT),
            "true",
            DesOmit::Eq("true"),
        ),
    ),
    fp(
        fg::F_EXT_TITLE_DATA_PATH,
        "titleDataPath",
        "TitleDataPath",
        Region::Ext,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    // backColor — Color Symmetric, метамодель после titleDataPath, ДО scrollOnCompress
    // (witnesses УдалениеПомеченныхОбъектов ×2 / НастройкиОчисткиФайлов; Designer-порядок
    // ShowTitle→BackColor→ScrollOnCompress уже в DES_GROUP_ORDER).
    fp(
        fg::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SCROLL_ON_COMPRESS,
        "scrollOnCompress",
        "ScrollOnCompress",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // format — Localized Symmetric (метамодель PageGroupExtInfo#9; ERP 9⟷9);
    // cf-ячейка НЕ витнесснута — типизированный отказ cf-write (page_ext).
    fp(
        fg::F_EXT_FORMAT,
        "format",
        "Format",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
];

/// extInfo ButtonGroup.
pub(crate) static BUTTON_GROUP_EXT: &[FieldProj] = &[
    fp(
        fg::F_EXT_COMMAND_SOURCE,
        "commandSource",
        "CommandSource",
        Region::Ext,
        Codec::RefText,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_REPRESENTATION,
        "representation",
        "Representation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// extInfo CommandBar (`horizontalAlign` ⟺ Designer `HorizontalLocation`, KEEP:
/// EDT опускает `Auto`, Designer — `Left`; cross-omission 1/16).
pub(crate) static COMMAND_BAR_EXT: &[FieldProj] = &[
    fp(
        fg::F_EXT_HORIZONTAL_ALIGN,
        "horizontalAlign",
        "HorizontalLocation",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::CB_HORIZONTAL_ALIGN_EDT_DEFAULT,
            Some(fg::CB_HORIZONTAL_ALIGN_EDT_DEFAULT),
            fg::CB_HORIZONTAL_ALIGN_DESIGNER_DEFAULT,
            DesOmit::Eq(fg::CB_HORIZONTAL_ALIGN_DESIGNER_DEFAULT),
        ),
    ),
    fp(
        fg::F_EXT_APPEARANCE_MODE,
        "appearanceMode",
        "AppearanceMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_COMMAND_SOURCE,
        "commandSource",
        "CommandSource",
        Region::Ext,
        Codec::RefText,
        Policy::Symmetric,
    ),
];

/// extInfo ColumnGroup (LANE-F-4): group/showTitle keep-пары (как UsualGroup),
/// showInCard/showTitleInCard симметричны, showInHeader opposite-bool, headerPicture — картинка.
/// EDT-порядок эмиссии = xcore-модель (`models/forms` ColumnGroupExtInfo): group(2) →
/// showTitle(3) → showInHeader(6) → headerPicture(10) → showTitleInCard(12) → showInCard(13) —
/// `showInCard` ЗАМЫКАЕТ. Сверено SSL edt: 25× `showInHeader→showInCard`, 2×
/// `headerPicture→showInCard`, 2× `showTitleInCard→showInCard`; контр-примеров 0.
pub(crate) static COLUMN_GROUP_EXT: &[FieldProj] = &[
    fp(
        fg::F_EXT_GROUP,
        "group",
        "Group",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::GROUP_EDT_DEFAULT,
            Some(fg::GROUP_EDT_DEFAULT),
            fg::COLUMN_GROUP_GROUP_DESIGNER_DEFAULT,
            DesOmit::Eq(fg::COLUMN_GROUP_GROUP_DESIGNER_DEFAULT),
        ),
    ),
    xml220(
        fp(
            fg::F_EXT_SHOW_TITLE,
            "showTitle",
            "ShowTitle",
            Region::Ext,
            Codec::EnumTok,
            keep(
                fg::SHOW_TITLE_EDT_DEFAULT,
                Some(fg::SHOW_TITLE_EDT_DEFAULT),
                fg::DESIGNER_AUTO_LOWER,
                DesOmit::Eq(fg::DESIGNER_AUTO_LOWER),
            ),
        ),
        keep(
            fg::SHOW_TITLE_EDT_DEFAULT,
            Some(fg::SHOW_TITLE_EDT_DEFAULT),
            "true",
            DesOmit::Eq("true"),
        ),
    ),
    fp(
        F_GRP_TITLE_BACK_COLOR,
        "titleBackColor",
        "TitleBackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // showInHeader: оба формата эмитят явный `true` (симметрично).
    fp(
        fg::F_EXT_SHOW_IN_HEADER,
        "showInHeader",
        "ShowInHeader",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_HEADER_PICTURE,
        "headerPicture",
        "HeaderPicture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SHOW_TITLE_IN_CARD,
        "showTitleInCard",
        "ShowTitleInCard",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // showInCard: EDT эмитит ВСЕГДА `true` (47/47) и ПОСЛЕДНИМ; Designer НЕ несёт (дефолт true).
    fp(
        fg::F_EXT_SHOW_IN_CARD,
        "showInCard",
        "ShowInCard",
        Region::Ext,
        Codec::Bool,
        KEEP_BOOL_TRUE,
    ),
    // headerHorizontalAlign — Symmetric presence-точно (ERP designer несёт ЯВНЫЕ
    // Left×21/Center×101/Right×7 — Designer НЕ опускает Left у ColumnGroup; fill-модель полей
    // сюда не переносится; SSL этот тег на ColumnGroup не несёт — аддитивно).
    // cf-ячейка НЕ витнесснута (s10 без ColumnGroup) — типизированный отказ cf-write.
    fp(
        ff::F_HEADER_HORIZONTAL_ALIGN,
        "headerHorizontalAlign",
        "HeaderHorizontalAlign",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // fixingInTable — Symmetric Enum (метамодель ColumnGroupExtInfo#10; ERP 55⟷(483
    // всего с полями)); cf-ячейка НЕ витнесснута — типизированный отказ cf-write.
    fp(
        ff::F_FIXING_IN_TABLE,
        "fixingInTable",
        "FixingInTable",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// extInfo Popup (`importance`: EDT эмитит ВСЕГДА, Designer никогда; `representation`:
/// EDT всегда, Designer опускает `Auto`).
pub(crate) static POPUP_EXT: &[FieldProj] = &[
    fp(
        fg::F_EXT_PICTURE,
        "picture",
        "Picture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    // representation: ПРОТИВОПОЛОЖНЫЕ дефолты (EDT опускает `Text`, Designer опускает `Auto`).
    fp(
        fg::F_EXT_REPRESENTATION,
        "representation",
        "Representation",
        Region::Ext,
        Codec::EnumTok,
        keep(
            fg::POPUP_REPRESENTATION_EDT_DEFAULT,
            Some(fg::POPUP_REPRESENTATION_EDT_DEFAULT),
            fg::DESIGNER_AUTO,
            DesOmit::Eq(fg::DESIGNER_AUTO),
        ),
    ),
    fp(
        fg::F_EXT_SHAPE,
        "shape",
        "Shape",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_SHAPE_REPRESENTATION,
        "shapeRepresentation",
        "ShapeRepresentation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        fg::F_EXT_COMMAND_SOURCE,
        "commandSource",
        "CommandSource",
        Region::Ext,
        Codec::RefText,
        Policy::Symmetric,
    ),
    // importance — как и `textSize`, ОБЫЧНАЯ пара пер-форматных дефолтов, а не нерегулярность.
    // EDT-дефолт = `Main` (EMF: первый литерал перечисления), Designer-дефолт = `Normal`. Единая
    // EDT-омиссия (МашиночитаемыеДоверенности.ФормаЭлемента Popup×1231) — это и есть `Main`:
    // контингентная таблица по всем 173 Popup'ам SSL ЧИСТАЯ, без внедиагональных клеток
    // (`<ABSENT>`⟺`Main` ×1, `Normal`⟺`Normal` ×169, `Supplementary`⟺`Supplementary` ×3 —
    // `probe_defaults`). ⇒ Policy::Keep: bag'и диалектов совпадают, оба R byte-exact.
    xml220(
        fp(
            fg::F_EXT_IMPORTANCE,
            "importance",
            "Importance",
            Region::Ext,
            Codec::EnumTok,
            Policy::Keep(Keep {
                edt_fill: fg::POPUP_IMPORTANCE_EDT_FILL,
                edt_omit: Some(fg::POPUP_IMPORTANCE_EDT_FILL),
                des_fill: fg::POPUP_IMPORTANCE_FILL,
                des_omit: DesOmit::Eq(fg::POPUP_IMPORTANCE_FILL),
            }),
        ),
        keep(
            fg::POPUP_IMPORTANCE_EDT_FILL,
            Some(fg::POPUP_IMPORTANCE_EDT_FILL),
            "Main",
            DesOmit::Eq("Main"),
        ),
    ),
    // backColor/borderColor — Color Symmetric (метамодель PopupGroupExtInfo#8/#9;
    // ERP 5⟷5 и 2⟷2). cf: popup-композит cells[7]/[8] (абляция s10).
    fp(
        fg::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        F_GRP_POPUP_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
];
