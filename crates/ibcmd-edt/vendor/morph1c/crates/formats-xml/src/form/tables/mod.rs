//! Проекционные ТАБЛИЦЫ контролов (LANE-F-2): FormField (общее тело + extInfo
//! InputField/CheckBoxField/LabelField) и семейство FormGroup (общее тело + extInfo
//! UsualGroup/Pages/Page/ButtonGroup). Порядок строк таблиц = КАНОНИЧЕСКИЙ порядок спека
//! (= EDT-порядок эмиссии, topo по корпусу, конфликтов 0); Designer-порядок — отдельные
//! слот-массивы ([`DES_FIELD_ORDER`], [`DES_GROUP_ORDER`], pooled topo, конфликтов 0).
//!
//! Политики/дефолты — свидетельства корпуса (cross-omission, см. `core/spec/forms/controls/
//! {form_field,form_group}` и).

use super::fields::{fp, Codec, DesOmit, FieldProj, Keep, Policy, Region};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::forms::controls::form_field as ff;

mod button_decoration;
mod catalog;
mod des_order;
mod field;
mod group;
mod table;

pub(crate) use button_decoration::*;
pub(crate) use catalog::*;
pub(crate) use des_order::*;
pub(crate) use field::*;
pub(crate) use group::*;
pub(crate) use table::*;

const fn keep(
    edt_fill: &'static str,
    edt_omit: Option<&'static str>,
    des_fill: &'static str,
    des_omit: DesOmit,
) -> Policy {
    Policy::Keep(Keep {
        edt_fill,
        edt_omit,
        des_fill,
        des_omit,
    })
}

/// [`Policy::PlatformDefault`] — как [`keep`], но канон НЕ хранит `des_fill` (дефолт
/// ПЛАТФОРМЫ), поэтому оба ридера дают ОДИН И ТОТ ЖЕ IR у незаданного свойства (X-канон).
/// Применимо ТОЛЬКО там, где `edt_fill != des_fill` и кросс-витнесс доказал, что
/// «Designer-омиссия» и «EDT-явный `des_fill`» — ОДНО состояние (см. док варианта).
const fn platform_default(
    edt_fill: &'static str,
    edt_omit: Option<&'static str>,
    des_fill: &'static str,
    des_omit: DesOmit,
) -> Policy {
    Policy::PlatformDefault(Keep {
        edt_fill,
        edt_omit,
        des_fill,
        des_omit,
    })
}

/// `des_attr`-вариант конструктора (Designer кодирует поле атрибутом элемента).
const fn fpa(
    id: FieldId,
    edt: &'static str,
    des: &'static str,
    codec: Codec,
    policy: Policy,
) -> FieldProj {
    FieldProj {
        id,
        edt,
        des,
        region: Region::Body,
        codec,
        policy,
        des_attr: true,
    }
}

// ============================ ЛОКАЛЬНЫЕ FieldId (ERP-волна) ============================
// Свойства, ВИТНЕССНУТЫЕ ERP-корпусом (11400 designer-форм + EDT-зеркала), которых нет в
// канонических спеках `core/spec/forms/controls` (core — вне зоны этой волны). Id-пространства
// пер-сущностные; выбраны из диапазона 900+ (свободен во всех форм-спеках core; drift-guard —
// коллизия внутри ОДНОЙ таблицы дала бы двойное чтение одного тега и §1.0-упала бы на
// тотальности). Зеркальные константы на cf-стороне: `formats/cf/src/form_body` (те же числа).
//
// Поля БЕЗ строки в core-спеке не сортируются `sort_props_by_spec` (стабильный sort оставляет
// их в порядке чтения = порядок таблицы, одинаковый в обоих ридерах ⇒ X-сравнимо).

/// Table: `useAlternationRowColor` ⟺ Designer `UseAlternationRowColor` — ПЛОСКИЙ bool
/// (метамодель Table#73; ОТДЕЛЬНОЕ поле от нуляемого твина `useAlternationRowColorBWA`#76).
/// ERP 3864×true ⟷ 3864×true (Symmetric). cf: TAIL[47] (false 0/true 1/absent 2) +
/// ДЕРИВАТ HEAD[36]=1 при true (абляция s10: оба у true, ТОЛЬКО TAIL[47] у false).
pub(super) const F_TB_USE_ALTERNATION_ROW_COLOR: FieldId = FieldId(901);
/// Table: `titleHeight` (метамодель Table#20; ERP 7×; cf HEAD[7], абляция s10 0→2).
pub(super) const F_TB_TITLE_HEIGHT: FieldId = FieldId(902);
/// Table: `shortcut` (метамодель Table#21; ERP 3×`Ctrl+…`; cf HEAD[51] {0,key,mods} —
/// абляция s10 Ctrl+1 → {0,49,8}).
pub(super) const F_TB_SHORTCUT: FieldId = FieldId(903);
/// Table: `refreshRequest` (метамодель Table#109; ERP 38×`PullFromTop` ⟷ 38; cf TAIL[21]:
/// absent 0 / PullFromTop 1 — абляция s10).
pub(super) const F_TB_REFRESH_REQUEST: FieldId = FieldId(904);
/// Table: `behaviorOnHorizontalCompression` (метамодель Table#111; ERP 1×
/// `MoveItemsByImportance` ⟷ 1; cf TAIL[33]: absent 0 / MoveItemsByImportance 2 — абляция s10).
pub(super) const F_TB_BEHAVIOR_ON_HORIZONTAL_COMPRESSION: FieldId = FieldId(905);

/// Addition (доб. Таблицы): `maxWidth` extInfo ⟺ Designer `MaxWidth` (метамодель
/// SearchString/ViewStatus AdditionExtInfo: autoMaxWidth→maxWidth; ERP 13⟷13 SearchString;
/// cf: композит-ячейка маркер+2 — абляция s10 0→20; SearchControl — cf-отказ, носителей нет).
pub(super) const F_ADDITION_MAX_WIDTH: FieldId = FieldId(921);
/// Addition: `horizontalLocation` extInfo ⟺ Designer `HorizontalLocation` — ТОЛЬКО у
/// ViewStatusAdditionExtInfo (метамодель #6, enum ItemHorizontalAlignment; ERP 3773⟷3773×Left).
/// cf: ViewStatus-композит, ячейка ЗА border-guid-записью: absent→3, Left→0 (абляция s10 +
/// erp.cf-witness Международный.ФормаСписка); Center/Right — cf-отказ (не витнесснуты).
pub(super) const F_ADDITION_HORIZONTAL_LOCATION: FieldId = FieldId(922);
/// Addition: `toolTipRepresentation` ТЕЛО ⟺ Designer `ToolTipRepresentation` (метамодель
/// Addition#5; ERP 9⟷9: None/Button). cf: addition-запись `{6,…}` cell[11]: absent 0 /
/// Button 3 (абляция s10; шкала = TAIL[9]-мэп Таблицы — одна платформенная enum).
pub(super) const F_ADDITION_TOOL_TIP_REPRESENTATION: FieldId = FieldId(923);
/// Addition: `toolTip` ТЕЛО (Localized; метамодель Addition#4; ERP 1× SearchString).
/// cf-ячейка НЕ витнесснута — типизированный отказ cf-write.
pub(super) const F_ADDITION_TOOL_TIP: FieldId = FieldId(924);
/// Addition: `displayImportance` ГОЛОВА (ЧИЛД после id) ⟺ Designer атрибут `DisplayImportance`
/// (witness DataProcessor.ЭлектронныеПеревозочныеДокументы.ФормаВыбораПолейПоиска
/// searchStringAddition `VeryHigh`). cf-ячейка addition-записи `{6,…}` для неё НЕ витнесснута ⇒
/// НЕ входит в `ADDITION_PROPS` cf-энкодера ⇒ типизированный отказ cf-write (§1.0).
pub(super) const F_ADDITION_DISPLAY_IMPORTANCE: FieldId = FieldId(925);
/// FormCommand: `use` — РОЛЕВОЕ ограничение команды (метамодель FormCommand#4,
/// AdjustableBoolean; ERP 391⟷391). Канон — `List([Bool(common), List([Str(role),
/// Bool(v)])…])`; ОТСУТСТВИЕ в bag ⟺ безролевой `<use><common>true</common></use>`
/// (EDT-константа) / отсутствие Designer `<Use>` — как прежде. cf: безролевой витнесс
/// `{"B",common}` (абляция s10 common-only 1→0); ролевые формы — типизированный cf-отказ.
pub(super) const F_CMD_USE: FieldId = FieldId(925);

/// FormField common: `titleBackColor` ⟺ Designer `TitleBackColor` (метамодель Field:
/// skipOnInput→titleBackColor→titleLocation; ERP 13× цветов; cf {48}-тело cell[33] —
/// абляция s10 #FCFCFC).
pub(super) const F_FF_TITLE_BACK_COLOR: FieldId = FieldId(941);
/// FormField common: `footerDataPath` ⟺ Designer `FooterDataPath` — DataPath-кодек
/// (EDT `<footerDataPath xsi:type="form:DataPath"><segments>…`; ERP 781×; cf {48}-тело
/// cell[12] — абляция s10 {0}→{1,{id}}).
pub(super) const F_FF_FOOTER_DATA_PATH: FieldId = FieldId(942);
/// FormField common: `footerTextColor` ⟺ Designer `FooterTextColor` (ERP 6×; cf {48}-тело
/// cell[34] — абляция s10 web:Green).
pub(super) const F_FF_FOOTER_TEXT_COLOR: FieldId = FieldId(943);
/// FormField common: `footerPicture` ⟺ Designer `FooterPicture` (PictureRef; ERP 2×;
/// cf-ячейка НЕ витнесснута — типизированный отказ cf-write).
pub(super) const F_FF_FOOTER_PICTURE: FieldId = FieldId(944);
/// BSP 8.5 source witness: Field widthInCard=Half, preserved without inferred defaults.
pub(super) const F_FF_WIDTH_IN_CARD: FieldId = FieldId(945);
/// BSP 8.5 source witness: Button onMainServerUnavalableBehavior=DontChangeBehavior.
pub(super) const F_BT_SERVER_UNAVAILABLE: FieldId = FieldId(946);

/// extInfo Input/LabelField: `markNegatives` ⟺ Designer `MarkNegatives` (метамодель
/// InputFieldExtInfo#18 / LabelFieldExtInfo#10; ERP 380⟷380, true×379+false×1; cf: Input
/// ext[10] / Label ext[5], тристейт false 0/true 1/absent 2 — абляция s10).
pub(super) const F_EXT_MARK_NEGATIVES: FieldId = FieldId(951);
/// extInfo InputField: `showCheckBoxesInDropList` (метамодель #17; ERP 1⟷1 true; cf Input
/// ext[19] absent 0/true 1 — абляция s10).
pub(super) const F_EXT_SHOW_CHECK_BOXES_IN_DROP_LIST: FieldId = FieldId(952);
/// extInfo InputField: `specialTextInputMode` (метамодель #70; ERP 8⟷8 Email/PhoneNumber/
/// Digits; cf Input ext[60]: absent 0, Email 4 — абляция s10; прочие литералы cf-НЕвитнесснуты).
pub(super) const F_EXT_SPECIAL_TEXT_INPUT_MODE: FieldId = FieldId(953);
/// extInfo SpreadsheetDocumentField: `viewScalingMode` ⟺ Designer `ViewScalingMode`
/// (ERP 1759⟷1759×Normal; cf ext[19]: absent 0 / Normal 1 — абляция s10).
pub(super) const F_EXT_VIEW_SCALING_MODE: FieldId = FieldId(955);
/// extInfo Radio/CheckBox: `itemWidth` ⟺ Designer `ItemWidth` (метамодель Radio#3 /
/// CheckBox#10; ERP 20⟷20; cf Radio ext[10] — абляция s10 0→8).
pub(super) const F_EXT_ITEM_WIDTH: FieldId = FieldId(956);
/// extInfo PictureField: `zoomable` ⟺ Designer `Zoomable` (ERP 9× true; cf-ячейка НЕ
/// витнесснута — типизированный отказ cf-write).
pub(super) const F_EXT_ZOOMABLE: FieldId = FieldId(957);
/// extInfo PictureField: `enableDrag` ⟺ Designer `EnableDrag` (ERP 2× true; cf-отказ).
pub(super) const F_EXT_PIC_ENABLE_DRAG: FieldId = FieldId(958);
/// extInfo PictureField: `borderColor` ⟺ Designer `BorderColor` (СОБСТВЕННЫЙ id, НЕ общий
/// `ff::F_EXT_BORDER_COLOR`: у PictureField своя позиция в пуловом Designer-порядке —
/// `ValuesPicture`→`BorderColor`→`Border` (witness Новости.ФормаНовости), тогда как общий
/// borderColor сидит у InputField-слота; EDT-порядок `border`→`borderColor`). Как и
/// `F_LD_BORDER_COLOR`/`F_GRP_POPUP_BORDER_COLOR` — локальный id ERP-волны, зеркалится в cf.
/// cf-ячейка НЕ витнесснута (SSL PictureField без borderColor) — эмит по witness-ключу.
pub(super) const F_PIC_BORDER_COLOR: FieldId = FieldId(959);
/// BSP 8.5 InputField choice button caption (localized pairs).
pub(super) const F_EXT_CHOICE_BUTTON_TITLE: FieldId = FieldId(960);
/// BSP 8.5 InputField drop-list hint (localized pairs).
pub(super) const F_EXT_DROP_LIST_HINT: FieldId = FieldId(961);

/// extInfo UsualGroup: `hiddenStateTitleBackColor` (метамодель UsualGroupExtInfo#28; ERP 1×;
/// cf {38}-композит cell[23] — абляция s10 style:BorderColor).
pub(super) const F_GRP_HIDDEN_STATE_TITLE_BACK_COLOR: FieldId = FieldId(971);
/// extInfo Popup: `borderColor` (метамодель PopupGroupExtInfo#9; ERP 2×; cf popup-композит
/// cells[8] — абляция s10 #FFFFFF).
pub(super) const F_GRP_POPUP_BORDER_COLOR: FieldId = FieldId(972);
/// extInfo LabelDecoration/ExtendedTooltip: `borderColor` (метамодель LabelDecorationExtInfo#6:
/// backColor(5)→borderColor(6)→border(7); ERP: декорации ×14 + подсказки ×1). cf: общий
/// LabelDecoration-ext `{5,…}`-суб-рекорд, ячейка [7] (абляция s10 web:Gainsboro /
/// style:BorderColor).
pub(super) const F_LD_BORDER_COLOR: FieldId = FieldId(981);
/// Тело декорации (Label+Picture): `shortcut` — комбинация клавиш (метамодель Decoration:
/// textColor→font→**shortcut**→groupHorizontalAlign; ERP witness PictureDecoration
/// НастройкиОбменаФСС.ФормаЗаписи `<Shortcut>X` / Мастер_ПаспортныеДанные). Локальный id
/// ERP-волны — cf-ячейка декорации НЕ витнесснута (SSL декорации без shortcut) ⇒ cf-writer
/// отвергает типизированно (`KNOWN_COMMON`), до майнинга оракула.
pub(super) const F_DEC_SHORTCUT: FieldId = FieldId(982);

/// Keep-политика «EDT эмитит всегда true; Designer опускает true» (`enabled`, `userVisible`).
const KEEP_BOOL_TRUE: Policy = keep("true", None, "true", DesOmit::Eq("true"));

// ---- Геометрический ПРЕФИКС extInfo. id/edt/des/codec ОДНОГО геометрического поля
// одинаковы во всех подтипах-полях ⇒ фиксируются здесь; за пер-табличными РАЗНЯТСЯ лишь
// ПОЛИТИКА и НАБОР/ПОРЯДОК присутствующих строк (DoNotMerge-ловушка:
// width Symmetric у InputField, но Keep у HTML/Progress/Spreadsheet; verticalStretch Symmetric
// у Progress/Input, но OppositeBool у HTML/Formatted/Text; часть таблиц ОПУСКАЕТ maxWidth/
// maxHeight). Поэтому НЕ общий срез, а per-field builder: каждый берёт СВОЮ Policy. Эмитимый
// FieldProj ПОБИТОВО равен прежнему литералу `fp(...)` (drift-guard + survey-оракул доказывают). ----
const fn geo_width(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_WIDTH,
        "width",
        "Width",
        Region::Ext,
        Codec::Int,
        p,
    )
}
const fn geo_auto_max_width(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_AUTO_MAX_WIDTH,
        "autoMaxWidth",
        "AutoMaxWidth",
        Region::Ext,
        Codec::Bool,
        p,
    )
}
const fn geo_max_width(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_MAX_WIDTH,
        "maxWidth",
        "MaxWidth",
        Region::Ext,
        Codec::Int,
        p,
    )
}
const fn geo_height(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_HEIGHT,
        "height",
        "Height",
        Region::Ext,
        Codec::Int,
        p,
    )
}
const fn geo_auto_max_height(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_AUTO_MAX_HEIGHT,
        "autoMaxHeight",
        "AutoMaxHeight",
        Region::Ext,
        Codec::Bool,
        p,
    )
}
const fn geo_max_height(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_MAX_HEIGHT,
        "maxHeight",
        "MaxHeight",
        Region::Ext,
        Codec::Int,
        p,
    )
}
const fn geo_h_stretch(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Ext,
        Codec::Bool,
        p,
    )
}
const fn geo_v_stretch(p: Policy) -> FieldProj {
    fp(
        ff::F_EXT_VERTICAL_STRETCH,
        "verticalStretch",
        "VerticalStretch",
        Region::Ext,
        Codec::Bool,
        p,
    )
}
