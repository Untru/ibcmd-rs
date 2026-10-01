//! extInfo простых полей ввода: Input / CheckBox / Label / RadioButton.

use crate::form::fields::{fp, Codec, DesOmit, FieldProj, Keep, Policy, Region};
use crate::form::tables::{
    geo_auto_max_height, geo_auto_max_width, geo_h_stretch, geo_height, geo_max_height,
    geo_max_width, geo_v_stretch, geo_width, keep, F_EXT_ITEM_WIDTH, F_EXT_MARK_NEGATIVES,
    F_EXT_SHOW_CHECK_BOXES_IN_DROP_LIST, F_EXT_SPECIAL_TEXT_INPUT_MODE,
    F_EXT_CHOICE_BUTTON_TITLE, F_EXT_DROP_LIST_HINT,
};
use morph1c_core::spec::forms::controls::form_field as ff;
use morph1c_core::spec::forms::controls::radio_button as rb;

/// extInfo InputField (порядок = канонический спек = ПОРЯДОК МЕТАМОДЕЛИ `InputFieldExtInfo`;
/// EDT эмитит extInfo строго в порядке метамодели — сверено 2917/2917 InputFieldExtInfo
/// корпусов SSL+coverage EDT, 0 нарушений; см. `core/spec/forms/controls/form_field.rs`).
pub(crate) static INPUT_FIELD_EXT: &[FieldProj] = &[
    geo_width(Policy::Symmetric),
    geo_auto_max_width(Policy::OppositeBool),
    geo_max_width(Policy::Symmetric),
    geo_height(Policy::Symmetric),
    geo_auto_max_height(Policy::OppositeBool),
    geo_max_height(Policy::Symmetric),
    geo_h_stretch(Policy::Symmetric),
    geo_v_stretch(Policy::Symmetric),
    fp(
        ff::F_EXT_WRAP,
        "wrap",
        "Wrap",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_PASSWORD_MODE,
        "passwordMode",
        "PasswordMode",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_MULTI_LINE,
        "multiLine",
        "MultiLine",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // extendedEdit — Symmetric Bool (оба формата эмитят true И явный false; SSL 33⟷33).
    fp(
        ff::F_EXT_EXTENDED_EDIT,
        "extendedEdit",
        "ExtendedEdit",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_ALLOW_INPUT_EMPTY_MULTIPLE_VALUES,
        "allowInputEmptyMultipleValues",
        "AllowInputEmptyMultipleValues",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_ALLOW_MULTIPLE_VALUES_DUPLICATES,
        "allowMultipleValuesDuplicates",
        "AllowMultipleValuesDuplicates",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_EXTENDED_EDIT_MULTIPLE_VALUES,
        "extendedEditMultipleValues",
        "ExtendedEditMultipleValues",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // showCheckBoxesInDropList — Symmetric Bool (метамодель InputFieldExtInfo#17; ERP 1⟷1
    // true; witness УправлениеПродажамиНаOzon ListChoiceMode→ShowCheckBoxesInDropList).
    // cf: Input ext[19] (абляция s10 0→1).
    fp(
        F_EXT_SHOW_CHECK_BOXES_IN_DROP_LIST,
        "showCheckBoxesInDropList",
        "ShowCheckBoxesInDropList",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // markNegatives — Symmetric Bool (метамодель InputFieldExtInfo#18; ERP 323⟷323).
    // cf: Input ext[10] тристейт (абляция s10 2→1).
    fp(
        F_EXT_MARK_NEGATIVES,
        "markNegatives",
        "MarkNegatives",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_LIST_BUTTON,
        "choiceListButton",
        "ChoiceListButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_DROP_LIST_BUTTON,
        "dropListButton",
        "DropListButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_BUTTON,
        "choiceButton",
        "ChoiceButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_BUTTON_REPRESENTATION,
        "choiceButtonRepresentation",
        "ChoiceButtonRepresentation",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        F_EXT_CHOICE_BUTTON_TITLE,
        "choiceButtonTitle",
        "ChoiceButtonTitle",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_PICTURE,
        "picture",
        "Picture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_BUTTON_PICTURE,
        "choiceButtonPicture",
        "ChoiceButtonPicture",
        Region::Ext,
        Codec::PictureRef,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CLEAR_BUTTON,
        "clearButton",
        "ClearButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SPIN_BUTTON,
        "spinButton",
        "SpinButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_OPEN_BUTTON,
        "openButton",
        "OpenButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CREATE_BUTTON,
        "createButton",
        "CreateButton",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_MASK,
        "mask",
        "Mask",
        Region::Ext,
        Codec::Text,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_AUTO_CHOICE_INCOMPLETE,
        "autoChoiceIncomplete",
        "AutoChoiceIncomplete",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_QUICK_CHOICE,
        "quickChoice",
        "QuickChoice",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_FOLDERS_AND_ITEMS,
        "choiceFoldersAndItems",
        "ChoiceFoldersAndItems",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_FORMAT,
        "format",
        "Format",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_EDIT_FORMAT,
        "editFormat",
        "EditFormat",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_LIST_CHOICE_MODE,
        "listChoiceMode",
        "ListChoiceMode",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_LIST_HEIGHT,
        "choiceListHeight",
        "ChoiceListHeight",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_AUTO_MARK_INCOMPLETE,
        "autoMarkIncomplete",
        "AutoMarkIncomplete",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOOSE_TYPE,
        "chooseType",
        "ChooseType",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_INCOMPLETE_CHOICE_MODE,
        "incompleteChoiceMode",
        "IncompleteChoiceMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // typeDomainEnabled/textSize: EDT эмитит ВСЕГДА, Designer НЕ несёт никогда (fill-константы
    // корпуса; дивергенция значений ловится X-сравнением, не маскируется).
    // typeDomainEnabled: OppositeBool (EDT эмитит `true`, опускает `false`; Designer эмитит
    // `false`, опускает `true`) — сверено корпусом покрытия (вариант `ТипДоменаВключен_Ложь`
    // пуст в EDT, а Designer несёт `<TypeDomainEnabled>false>`).
    fp(
        ff::F_EXT_TYPE_DOMAIN_ENABLED,
        "typeDomainEnabled",
        "TypeDomainEnabled",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_TEXT_EDIT,
        "textEdit",
        "TextEdit",
        Region::Ext,
        Codec::Bool,
        Policy::OppositeBool,
    ),
    fp(
        ff::F_EXT_EDIT_TEXT_UPDATE,
        "editTextUpdate",
        "EditTextUpdate",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // minValue/maxValue — nullable-скаляр xsi-вида через ОБЩИЙ value_codec (Codec::Value):
    // ERP несёт Number-ДЕСЯТИЧНЫЙ (`99.99`/`0.001`) И `core:StringValue` (`"1"`), чего узкий
    // Int-кодек (core:NumberValue-int) не покрывал (класс form value-типов). Канон —
    // `PropertyValue::Value(ValueSpec)`; cf-ячейка `{"N",n}`/`{"S",s}`/`{"U"}` (brace value_codec).
    fp(
        ff::F_EXT_MIN_VALUE,
        "minValue",
        "MinValue",
        Region::Ext,
        Codec::Value,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_MAX_VALUE,
        "maxValue",
        "MaxValue",
        Region::Ext,
        Codec::Value,
        Policy::Symmetric,
    ),
    // choiceForm: форма выбора (Ref-текст; метамодель #47 — maxValue → choiceForm →
    // choiceParameterLinks; witness МашиночитаемыеДоверенности textEdit→choiceForm→textSize).
    fp(
        ff::F_EXT_CHOICE_FORM,
        "choiceForm",
        "ChoiceForm",
        Region::Ext,
        Codec::RefText,
        Policy::Symmetric,
    ),
    // choiceParameterLinks: связи параметров выбора (repeatable, structured; метамодель #48).
    fp(
        ff::F_EXT_CHOICE_PARAMETER_LINKS,
        "choiceParameterLinks",
        "ChoiceParameterLinks",
        Region::Ext,
        Codec::ChoiceParameterLinks,
        Policy::Symmetric,
    ),
    // choiceParameters: параметры выбора (repeatable, FormChoiceListDesTimeValue-обёртка);
    // метамодель: choiceParameters 49 → availableTypes 50 → choiceList 51.
    fp(
        ff::F_EXT_CHOICE_PARAMETERS,
        "choiceParameters",
        "ChoiceParameters",
        Region::Ext,
        Codec::ChoiceParameters,
        Policy::Symmetric,
    ),
    // availableTypes: тип-значение через общий type-codec (EDT `<availableTypes><types>…` ⟺
    // Designer `<AvailableTypes><v8:Type>…`); симметрично.
    fp(
        ff::F_EXT_AVAILABLE_TYPES,
        "availableTypes",
        "AvailableTypes",
        Region::Ext,
        Codec::Type,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_LIST,
        "choiceList",
        "ChoiceList",
        Region::Ext,
        Codec::ChoiceList,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // borderColor — Symmetric Color (SSL 42⟷42, значения зеркальны: Style.*⟷style:*, Palette.*⟷pal:*).
    fp(
        ff::F_EXT_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    // textSize: ОБЫЧНАЯ пер-форматная пара дефолтов, а НЕ нерегулярность. Прежняя волна читала
    // EDT-омиссию (82/2820) как «БЕЗ разрешимого правила» и хранила EDT-сторону presence-точно
    // (`Policy::DesKeep`) — из-за чего IR диалектов расходился и designer→cf ОТКАЗЫВАЛ на
    // `Enlarged`. Правило есть, и оно чистое: EDT-дефолт = `Enlarged` (EMF: дефолт EEnum-атрибута
    // = первый литерал), Designer-дефолт = `Normal`. Контингентная таблица (EDT × Designer) по всем
    // 2820 InputField'ам SSL — РОВНО две клетки, НИ ОДНОЙ внедиагональной: `<ABSENT>`⟺`Enlarged`
    // (82), `Normal`⟺`Normal` (2738). См. `probe_defaults` + `ff::TEXT_SIZE_EDT_FILL`.
    // ⇒ Policy::Keep: каждый диалект заполняет СВОЙ дефолт на чтении и опускает его на записи;
    // bag'и совпадают (X-равны), оба R остаются byte-exact.
    fp(
        ff::F_EXT_TEXT_SIZE,
        "textSize",
        "TextSize",
        Region::Ext,
        Codec::EnumTok,
        Policy::Keep(Keep {
            edt_fill: ff::TEXT_SIZE_EDT_FILL,
            edt_omit: Some(ff::TEXT_SIZE_EDT_FILL),
            des_fill: ff::TEXT_SIZE_FILL,
            des_omit: DesOmit::Eq(ff::TEXT_SIZE_FILL),
        }),
    ),
    // typeLink: связь по типу (structured; метамодель #63 — ПОСЛЕ textSize (55), ДО
    // heightControlVariant (64); witness EDT textSize→typeLink→КОНЕЦ extInfo ×7).
    fp(
        F_EXT_DROP_LIST_HINT,
        "dropListHint",
        "DropListHint",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_TYPE_LINK,
        "typeLink",
        "TypeLink",
        Region::Ext,
        Codec::TypeLink,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_HEIGHT_CONTROL_VARIANT,
        "heightControlVariant",
        "HeightControlVariant",
        Region::Ext,
        Codec::EnumMap(&[
            ("InFormRows", "UseHeightInFormRows"),
            ("ByContent", "UseContentHeight"),
            ("InTableRows", "UseHeightInTableRows"),
        ]),
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_AUTO_SHOW_OPEN_BUTTON_MODE,
        "autoShowOpenButtonMode",
        "AutoShowOpenButtonMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_AUTO_SHOW_CLEAR_BUTTON_MODE,
        "autoShowClearButtonMode",
        "AutoShowClearButtonMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_AUTO_CORRECTION_ON_TEXT_INPUT,
        "autoCorrectionOnTextInput",
        "AutoCorrectionOnTextInput",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_SPELL_CHECKING_ON_TEXT_INPUT,
        "spellCheckingOnTextInput",
        "SpellCheckingOnTextInput",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    // specialTextInputMode — Symmetric Enum (метамодель InputFieldExtInfo#70:
    // spellCheckingOnTextInput(68)→…→specialTextInputMode(70)→…→inputHint(73); ERP 8⟷8:
    // Email×5, PhoneNumber×2, Digits×1). cf: Input ext[60] — витнесснут ТОЛЬКО Email=4
    // (абляция s10); прочие литералы — типизированный cf-отказ.
    fp(
        F_EXT_SPECIAL_TEXT_INPUT_MODE,
        "specialTextInputMode",
        "SpecialTextInputMode",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_INPUT_HINT,
        "inputHint",
        "InputHint",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_MULTIPLE_VALUE_DATA_PATH,
        "multipleValueDataPath",
        "MultipleValueDataPath",
        Region::Ext,
        Codec::DataPath,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_DROP_LIST_WIDTH,
        "dropListWidth",
        "DropListWidth",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_CHOICE_HISTORY_ON_INPUT,
        "choiceHistoryOnInput",
        "ChoiceHistoryOnInput",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
];

/// extInfo CheckBoxField.
pub(crate) static CHECK_BOX_FIELD_EXT: &[FieldProj] = &[
    fp(
        ff::F_EXT_CHECK_BOX_TYPE,
        "checkBoxType",
        "CheckBoxType",
        Region::Ext,
        Codec::EnumTok,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_THREE_STATE,
        "threeState",
        "ThreeState",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_EDIT_FORMAT,
        "editFormat",
        "EditFormat",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    // Хвост метамодели CheckBoxFieldExtInfo: editFormat(8)→itemTitleHeight(9)→itemWidth(10)→
    // itemHeight(11)→equalElementsWidth(12). ERP-witness ВидыНоменклатуры/ВидыРабочихЦентров.
    // rb::-id'ы РЕИСПОЛЬЗУЮТСЯ (те же пуловые DES-слоты); ItemWidth — локальный id (см. блок 900+).
    // cf-ячейки CheckBox-ext: itemTitleHeight [8], itemHeight [10], equalItemsWidth [11]
    // (тристейт), itemWidth — cf-ячейка НЕ витнесснута (типизированный отказ).
    fp(
        rb::F_EXT_ITEM_TITLE_HEIGHT,
        "itemTitleHeight",
        "ItemTitleHeight",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        F_EXT_ITEM_WIDTH,
        "itemWidth",
        "ItemWidth",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_ITEM_HEIGHT,
        "itemHeight",
        "ItemHeight",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    // equalElementsWidth ⟷ Designer `EqualItemsWidth` (у CheckBox — Items, у Radio —
    // Columns; разноимённые Designer-теги при ОДНОМ EDT-теге). ERP 13⟷13 (false×10, true×3).
    fp(
        rb::F_EXT_EQUAL_ELEMENTS_WIDTH,
        "equalElementsWidth",
        "EqualItemsWidth",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
];

/// extInfo LabelField (Designer-тег `Hiperlink` — опечатка платформы, сверено по корпусу).
/// Порядок хвоста = МЕТАМОДЕЛЬ `LabelFieldExtInfo` (format, hyperlink, passwordMode, border,
/// textColor, backColor, useCopy) — прежний (textColor/backColor ДО format/hyperlink) был
/// tie-break-артефактом topo; SSL-витнессы метамодель подтверждают: hyperlink<passwordMode<
/// textColor (ХранилищеВариантовОтчетов), verticalStretch<border<backColor
/// (ПоискИУдалениеДублей), hyperlink<textColor, border<backColor; контрпримеров 0.
pub(crate) static LABEL_FIELD_EXT: &[FieldProj] = &[
    geo_width(Policy::Symmetric),
    geo_auto_max_width(Policy::OppositeBool),
    geo_max_width(Policy::Symmetric),
    geo_height(Policy::Symmetric),
    geo_auto_max_height(Policy::OppositeBool),
    geo_max_height(Policy::Symmetric),
    // horizontalStretch — после auto/maxHeight-геометрии, ДО format/hyperlink/цветов
    // (SSL LabelFieldExtInfo: autoMaxHeight/autoMaxWidth/width<hStretch, hStretch<format×16/
    // hyperlink×6/textColor×2/backColor×1).
    geo_h_stretch(Policy::Symmetric),
    // verticalStretch — сразу после horizontalStretch (метамодель геометрии; witness
    // ПереносФайлов LabelFieldExtInfo autoMaxHeight→verticalStretch).
    geo_v_stretch(Policy::Symmetric),
    // markNegatives — Symmetric Bool (метамодель LabelFieldExtInfo: verticalStretch(9)→
    // markNegatives(10)→format(11); ERP 57⟷57×true). cf: Label ext[5] тристейт (абляция s10).
    fp(
        F_EXT_MARK_NEGATIVES,
        "markNegatives",
        "MarkNegatives",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_FORMAT,
        "format",
        "Format",
        Region::Ext,
        Codec::Localized,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_HYPERLINK,
        "hyperlink",
        "Hiperlink",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // passwordMode — Bool Symmetric (оба эмитят явные false; witness ХранилищеВариантовОтчетов
    // hyperlink→passwordMode→textColor ⟷ Hiperlink→PasswordMode→TextColor).
    fp(
        ff::F_EXT_PASSWORD_MODE,
        "passwordMode",
        "PasswordMode",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // border — Border-композит, Symmetric (оба эмитят не-дефолт; witness ПоискИУдалениеДублей
    // Single⟷Single). СОБСТВЕННЫЙ id (Designer-слот LabelField ≠ слот PictureField).
    fp(
        ff::F_EXT_LABEL_BORDER,
        "border",
        "Border",
        Region::Ext,
        Codec::Border,
        Policy::Symmetric,
    ),
    // borderColor — Color Symmetric (метамодель LabelFieldExtInfo: border(14)→borderColor(15)→
    // textColor(16); ERP 3⟷3). cf: Label ext[13] (абляция s10 web:Gainsboro).
    fp(
        ff::F_EXT_BORDER_COLOR,
        "borderColor",
        "BorderColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_BACK_COLOR,
        "backColor",
        "BackColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        ff::F_EXT_USE_COPY,
        "useCopy",
        "UseCopy",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
];

/// extInfo RadioButtonField (`radioButtonsType`⟺`RadioButtonType` KEEP Auto; `columnsCount`
/// симметричен; `choiceList`⟺`ChoiceList` structured; `orientation` KEEP Vertical — EDT всегда,
/// Designer опускает). Порядок = канонический EDT (topo 18/18).
pub(crate) static RADIO_BUTTON_FIELD_EXT: &[FieldProj] = &[
    fp(
        rb::F_EXT_RADIO_BUTTON_TYPE,
        "radioButtonsType",
        "RadioButtonType",
        Region::Ext,
        Codec::EnumTok,
        keep(
            rb::RADIO_BUTTON_TYPE_DEFAULT,
            Some(rb::RADIO_BUTTON_TYPE_DEFAULT),
            rb::RADIO_BUTTON_TYPE_DEFAULT,
            DesOmit::Never,
        ),
    ),
    // itemWidth — Symmetric Int (метамодель RadioButtonsFieldExtInfo: radioButtonsType(1)→
    // itemWidth(3)→itemHeight(4); ERP-witness НастройкиРасчетаРезервовПоОплатеТруда
    // RadioButtonType→ItemWidth→EqualColumnsWidth). cf: Radio ext[10] (абляция s10 0→8).
    fp(
        F_EXT_ITEM_WIDTH,
        "itemWidth",
        "ItemWidth",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_ITEM_HEIGHT,
        "itemHeight",
        "ItemHeight",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_ITEM_TITLE_HEIGHT,
        "itemTitleHeight",
        "ItemTitleHeight",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_COLUMNS_COUNT,
        "columnsCount",
        "ColumnsCount",
        Region::Ext,
        Codec::Int,
        Policy::Symmetric,
    ),
    // equalElementsWidth ⟷ Designer EqualColumnsWidth (разноимённые теги) — Bool Symmetric
    // (метамодель columnsCount→equalElementsWidth→choiceList; witness ПомощникСозданияОбменаДанными).
    fp(
        rb::F_EXT_EQUAL_ELEMENTS_WIDTH,
        "equalElementsWidth",
        "EqualColumnsWidth",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_CHOICE_LIST,
        "choiceList",
        "ChoiceList",
        Region::Ext,
        Codec::ChoiceList,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_HORIZONTAL_STRETCH,
        "horizontalStretch",
        "HorizontalStretch",
        Region::Ext,
        Codec::Bool,
        Policy::Symmetric,
    ),
    // textColor — Color Symmetric (метамодель RadioButtonsFieldExtInfo: font(10)→textColor(11);
    // ERP 3⟷3, значения зеркальны). cf: Radio ext[3] (абляция s10 #3BB371). РЕИСПОЛЬЗУЕТ
    // ff::F_EXT_TEXT_COLOR (пуловый DES-слот InputField textColor; Radio-порядок designer
    // не витнессирован — loose).
    fp(
        ff::F_EXT_TEXT_COLOR,
        "textColor",
        "TextColor",
        Region::Ext,
        Codec::Color,
        Policy::Symmetric,
    ),
    fp(
        rb::F_EXT_ORIENTATION,
        "orientation",
        "Orientation",
        Region::Ext,
        Codec::EnumTok,
        keep(
            rb::ORIENTATION_EDT_DEFAULT,
            Some(rb::ORIENTATION_EDT_DEFAULT),
            rb::ORIENTATION_DESIGNER_DEFAULT,
            DesOmit::Eq(rb::ORIENTATION_DESIGNER_DEFAULT),
        ),
    ),
];

