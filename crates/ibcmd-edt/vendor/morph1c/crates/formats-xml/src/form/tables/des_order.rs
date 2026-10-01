//! Designer-порядки эмиссии (слот-массивы `DES_*_ORDER`) + `DesSlot`-энум.

use crate::form::tables::{
    F_DEC_SHORTCUT, F_EXT_ITEM_WIDTH, F_EXT_MARK_NEGATIVES, F_EXT_SHOW_CHECK_BOXES_IN_DROP_LIST,
    F_EXT_SPECIAL_TEXT_INPUT_MODE, F_EXT_VIEW_SCALING_MODE, F_FF_FOOTER_DATA_PATH,
    F_FF_FOOTER_PICTURE, F_FF_FOOTER_TEXT_COLOR, F_FF_TITLE_BACK_COLOR,
    F_GRP_HIDDEN_STATE_TITLE_BACK_COLOR, F_GRP_POPUP_BORDER_COLOR, F_LD_BORDER_COLOR,
    F_PIC_BORDER_COLOR, F_FF_WIDTH_IN_CARD, F_BT_SERVER_UNAVAILABLE,
    F_EXT_CHOICE_BUTTON_TITLE, F_EXT_DROP_LIST_HINT,
};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::forms::controls::button as bt;
use morph1c_core::spec::forms::controls::form_field as ff;
use morph1c_core::spec::forms::controls::form_group as fg;
use morph1c_core::spec::forms::controls::label_decoration as ld;
use morph1c_core::spec::forms::controls::radio_button as rb;

// ============================ Designer-порядки (слоты) ============================

/// Слот Designer-эмиссии: поле по id либо каркас-маркер.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DesSlot {
    /// Поле (lookup в таблице контрола; отсутствует у вида — слот пропускается).
    F(FieldId),
    /// `<AutoEditMode>true` (пара канонического `editMode == Auto`).
    AutoEditMode,
    /// `<Title formatted="…">` декорации (glue: formatted — атрибут Designer-Title).
    TitleFormatted,
    /// `<Font ref= … kind=…/>` контрола (композит-шрифт, если задан).
    Font,
    /// `<TitleFont ref= … kind=…/>` — композит-шрифт ЗАГОЛОВКА контрола (если задан;
    /// witness LabelField/InputField/UsualGroup — сразу после `<Title>`).
    TitleFont,
    /// `<FooterFont ref= … kind=…/>` — шрифт ПОДВАЛА колонки (ERP-волна; witness
    /// РабочееМестоМенеджераПоДоставке: FooterTextColor→FooterFont→Width).
    FooterFont,
    /// `<ContextMenu …>` (decorator).
    ContextMenu,
    /// `<ExtendedTooltip …>` (decorator).
    ExtendedTooltip,
    /// `<Events>` контрола.
    Events,
    /// `<ChildItems>` (рекурсивные дети контейнера).
    ChildItems,
    /// `<CommandSet><ExcludedCommand>` — СОБСТВЕННЫЕ исключённые команды Таблицы.
    TableCommandSet,
    /// `<ShowCommandBar>значение` — три-состояние показа командной панели Таблицы
    /// (`{true, false, auto}`; из [`super::FormItem::show_command_bar`]).
    TableShowCommandBar,
    /// `<AutoCommandBar>` — СОБСТВЕННАЯ командная панель Таблицы.
    TableAutoCommandBar,
    /// Добавления Таблицы (`SearchStringAddition`/`ViewStatusAddition`/`SearchControlAddition`).
    /// Также несёт единственное `ViewStatusAddition` PDFDocumentField (из [`super::FormItem::additions`]).
    TableAdditions,
    /// АВТО-ТАБЛИЦА GanttChartField (`<Table>`-ребёнок; из [`super::FormItem::auto_table`]).
    /// Отсутствует ⇒ слот пропускается (no-op у всех видов, кроме GanttChartField).
    AutoTable,
    /// Инлайн-поля `form:DynamicListTableExtInfo` Таблицы-динамического-списка (`AutoRefresh`/
    /// `Period`/`TopLevelParent`/`ShowRoot`/…; из [`super::FormItem::dynamic_list_ext`]). Весь
    /// блок эмитится КОНТИГУОЗНО в [`DES_DYNAMIC_LIST_ORDER`]; отсутствует ⇒ слот пропускается.
    TableDynamicList,
}

/// Designer-порядок эмиссии Button (topo SSL, конфликтов 0; `Font` не смоделирован).
/// Голова пересобрана по полному pairwise-topo SSL (Visible<Enabled×9, Representation<Enabled×2,
/// DefaultButton<Enabled×1, DefaultButton<AutoMaxHeight×1, SkipOnInput<Enabled×3,
/// Width<AutoMaxWidth×3, AutoMaxWidth<AutoMaxHeight×2, AutoMaxWidth<Height×1,
/// Height<HorizontalStretch×1, HorizontalStretch<GroupHorizontalAlign×2,
/// GroupHorizontalAlign<GroupVerticalAlign×2; 0 контрпримеров — прежняя голова
/// AutoMaxHeight/Enabled-первыми ломала Type→Visible→Enabled, напр. ВнешниеПользователи).
pub(crate) static DES_BUTTON_ORDER: &[DesSlot] = &[
    DesSlot::F(bt::F_BUTTON_TYPE),
    DesSlot::F(bt::F_USER_VISIBLE),
    DesSlot::F(bt::F_VISIBLE),
    DesSlot::F(bt::F_REPRESENTATION),
    DesSlot::F(bt::F_DEFAULT_BUTTON),
    DesSlot::F(bt::F_SKIP_ON_INPUT),
    DesSlot::F(bt::F_ENABLED),
    DesSlot::F(bt::F_DEFAULT_ITEM),
    // TitleHeight — в геометрическом блоке ДО Width/AutoMaxWidth (witness
    // УдалениеПомеченныхОбъектов «НастроитьОтбор»: Type→TitleHeight→AutoMaxWidth→MaxWidth;
    // прежний слот после Title ломал порядок и терял пары).
    DesSlot::F(bt::F_TITLE_HEIGHT),
    DesSlot::F(bt::F_WIDTH),
    DesSlot::F(bt::F_AUTO_MAX_WIDTH),
    // MaxWidth — после AutoMaxWidth, ДО Height (witness AutoMaxWidth→MaxWidth→Height;
    // прежде слот ОТСУТСТВОВАЛ — Designer РОНЯЛ прочитанный <MaxWidth>).
    DesSlot::F(bt::F_MAX_WIDTH),
    DesSlot::F(bt::F_AUTO_MAX_HEIGHT),
    DesSlot::F(bt::F_MAX_HEIGHT),
    DesSlot::F(bt::F_HEIGHT),
    DesSlot::F(bt::F_HORIZONTAL_STRETCH),
    DesSlot::F(bt::F_VERTICAL_STRETCH),
    DesSlot::F(bt::F_GROUP_HORIZONTAL_ALIGN),
    // check — ПОСЛЕ GroupHorizontalAlign (witness РедактированиеТабличногоДокумента:
    // GroupHorizontalAlign→Check→CommandName ×1; прочие Check-витнессы позицию не сужают).
    DesSlot::F(bt::F_CHECK),
    DesSlot::F(bt::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(bt::F_COMMAND_NAME),
    // Parameter — сразу после CommandName (witness ×6: CommandName→Parameter→Title/
    // ExtendedTooltip; метамодель commandName→parameter; пара с DataPath не витнессирована —
    // tie-break по метамодели).
    DesSlot::F(bt::F_PARAMETER),
    // DataPath — сразу после CommandName, до Title/LocationInCommandBar (SSL 52/52,
    // напр. `CommandName > DataPath > Title`; конфликтов 0).
    DesSlot::F(bt::F_DATA_PATH),
    DesSlot::Font,
    // Цвета ДО Picture (SSL Designer: TextColor<BackColor×9, BackColor<BorderColor×9,
    // TextColor<BorderColor×49, BackColor<Picture×1, BorderColor<Picture×1, Font<Picture×6;
    // Picture<цвета НЕ витнессирован — прежняя позиция Picture-до-цветов была tie-break-слепой).
    DesSlot::F(bt::F_TEXT_COLOR),
    DesSlot::F(bt::F_BACK_COLOR),
    DesSlot::F(bt::F_BORDER_COLOR),
    DesSlot::F(bt::F_PICTURE),
    DesSlot::F(bt::F_TITLE),
    // `<Shape>`/`<PictureLocation>` — сразу после `<Title>` (witness Кнопка_Форма/РасположениеКартинки).
    DesSlot::F(bt::F_SHAPE),
    DesSlot::F(bt::F_PICTURE_LOCATION),
    // ToolTipRepresentation — РАННИЙ хвост-слот (SSL Designer: Title<TTR×97, TTR<
    // ButtonImportance×29, TTR<ShapeRepresentation×12, TTR<RepresentationInContextMenu×1,
    // TTR<LocationInCommandBar×1; НИЧЕГО-кроме-Title<TTR не витнессировано — прежние позиции
    // после LICB/RICM вскрыты СертификатыКлючей.ФормаЭлемента и МЧД.ФормаСписка).
    DesSlot::F(bt::F_TOOL_TIP_REPRESENTATION),
    DesSlot::F(bt::F_COMMAND_UNIQUENESS),
    DesSlot::F(bt::F_SHOW_AS_CARD),
    DesSlot::F(bt::F_REPRESENTATION_IN_CONTEXT_MENU),
    DesSlot::F(bt::F_SHAPE_REPRESENTATION),
    DesSlot::F(bt::F_LOCATION_IN_COMMAND_BAR),
    DesSlot::F(F_BT_SERVER_UNAVAILABLE),
    DesSlot::F(bt::F_BUTTON_IMPORTANCE),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    DesSlot::Events,
];

/// Designer-порядок эмиссии LabelDecoration (topo 157/157; `Border`/`Font` не смоделированы;
/// `UserVisible`/`Enabled` в Designer-корпусе не витнессированы — слоты перед `Visible`).
pub(crate) static DES_LABEL_DECORATION_ORDER: &[DesSlot] = &[
    DesSlot::F(ld::F_USER_VISIBLE),
    DesSlot::F(ld::F_ENABLED),
    DesSlot::F(ld::F_VISIBLE),
    DesSlot::F(ld::F_WIDTH),
    DesSlot::F(ld::F_AUTO_MAX_WIDTH),
    // MaxWidth — ДО Height (SSL LabelDecoration: AutoMaxWidth<MaxWidth×38, MaxWidth<Height×1
    // РезультатыОбновленияПрограммы; 0 контрпримеров — прежний Height-первым был tie-break).
    DesSlot::F(ld::F_MAX_WIDTH),
    DesSlot::F(ld::F_HEIGHT),
    DesSlot::F(ld::F_AUTO_MAX_HEIGHT),
    // maxHeight — СРАЗУ после AutoMaxHeight, ДО HorizontalStretch (SSL: AutoMaxHeight<
    // MaxHeight×2, MaxHeight<HorizontalStretch×1, MaxWidth<MaxHeight×1; witness
    // НастройкиПользователей.ОчисткаНастроекПользователей «ВыбратьНастройки»). Прежде слот
    // ОТСУТСТВОВАЛ в Designer-порядке ⇒ Designer РОНЯЛ прочитанный `<MaxHeight>`.
    DesSlot::F(ld::F_MAX_HEIGHT),
    DesSlot::F(ld::F_HORIZONTAL_STRETCH),
    // verticalStretch — МЕЖДУ HorizontalStretch и SkipOnInput (SSL Designer:
    // HorizontalStretch<VerticalStretch×22, VerticalStretch<SkipOnInput×5, VerticalStretch<
    // Border×3/TextColor×2/Title×29, 0 контрпримеров; напр. ДополнительныеРеквизитыИСведения.
    // РазблокированиеРеквизитов). Прежняя позиция после TextColor ломала
    // VerticalStretch→SkipOnInput.
    DesSlot::F(ld::F_VERTICAL_STRETCH),
    DesSlot::F(ld::F_SKIP_ON_INPUT),
    DesSlot::F(ld::F_TEXT_COLOR),
    // `<Font>` эмитится ПОСЛЕ геометрии, ПЕРЕД `<Title>` (corpus fact).
    DesSlot::Font,
    // shortcut — ПЕРЕД `<Title>` (декорации-witness: Shortcut первым ребёнком, до Title;
    // метамодель font→shortcut). LabelDecoration в SSL shortcut не несёт (слот безвреден).
    DesSlot::F(F_DEC_SHORTCUT),
    DesSlot::TitleFormatted,
    DesSlot::F(ld::F_EXT_TITLE_HEIGHT),
    DesSlot::F(ld::F_EXT_BACK_COLOR),
    // BorderColor — за BackColor (метамодель; ERP loose).
    DesSlot::F(F_LD_BORDER_COLOR),
    DesSlot::F(ld::F_TOOL_TIP),
    DesSlot::F(ld::F_TOOL_TIP_REPRESENTATION),
    // GroupHorizontalAlign→GroupVerticalAlign — ПОСЛЕ ToolTip/ToolTipRepresentation, ДО
    // Hyperlink/HorizontalAlign/VerticalAlign (SSL: Title<GHA×13/GVA×65, ToolTipRepresentation<
    // GHA×1/GVA×1, ToolTip<GVA×1, GHA<GVA×2, GVA<VerticalAlign×23, GHA<Hyperlink×8,
    // GVA<Hyperlink×6, GHA/GVA<HorizontalAlign; 0 контрпримеров — прежний порядок GVA-до-ToolTip
    // и GVA-до-GHA ломал Title→GHA→GVA УдалениеПомеченныхОбъектов).
    DesSlot::F(ld::F_GROUP_HORIZONTAL_ALIGN),
    DesSlot::F(ld::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(ld::F_EXT_HYPERLINK),
    DesSlot::F(ld::F_EXT_HORIZONTAL_ALIGN),
    DesSlot::F(ld::F_EXT_VERTICAL_ALIGN),
    // `<Border>` — ЗАМЫКАЮЩЕЕ поле, НЕПОСРЕДСТВЕННО перед ContextMenu (ВСЕ 4 SSL-витнесса:
    // SkipOnInput→Border→ContextMenu ПанельОтчетов; VerticalAlign→Border→ContextMenu ×2
    // ПоискИУдалениеДублей; Title→Border→ContextMenu НастройкиРаботыСФайловымАрхивом).
    // Прежняя позиция после SkipOnInput (до TextColor/Title) ломала Title/VerticalAlign→Border.
    DesSlot::F(ld::F_EXT_BORDER),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    DesSlot::Events,
];

/// Designer-порядок эмиссии PictureDecoration (topo 100/100).
pub(crate) static DES_PICTURE_DECORATION_ORDER: &[DesSlot] = &[
    DesSlot::F(ld::F_USER_VISIBLE),
    DesSlot::F(ld::F_ENABLED),
    DesSlot::F(ld::F_VISIBLE),
    // ПОЛНЫЙ геометрический блок ДО SkipOnInput (SSL PictureDecoration: Width<MaxWidth×1,
    // MaxWidth<Height×1/MaxHeight×2, Height<AutoMaxHeight×2/MaxHeight×3/HorizontalStretch×3,
    // AutoMaxWidth<AutoMaxHeight×3/HorizontalStretch×1, AutoMaxHeight<MaxHeight×2/
    // VerticalStretch×1, HorizontalStretch<VerticalStretch×2, гео<SkipOnInput ×8 суммарно;
    // 0 контрпримеров — прежде MaxWidth/AutoMax*/MaxHeight/HorizontalStretch-слоты
    // ОТСУТСТВОВАЛИ (Designer РОНЯЛ значения, напр. ПомощникСозданияОбменаДанными.
    // НоваяСинхронизацияДанных MaxWidth=80), а SkipOnInput стоял ДО геометрии).
    DesSlot::F(ld::F_WIDTH),
    DesSlot::F(ld::F_AUTO_MAX_WIDTH),
    DesSlot::F(ld::F_MAX_WIDTH),
    DesSlot::F(ld::F_HEIGHT),
    DesSlot::F(ld::F_AUTO_MAX_HEIGHT),
    DesSlot::F(ld::F_MAX_HEIGHT),
    DesSlot::F(ld::F_HORIZONTAL_STRETCH),
    DesSlot::F(ld::F_VERTICAL_STRETCH),
    DesSlot::F(ld::F_SKIP_ON_INPUT),
    // TextColor — ПЕРЕД Font (witness ТомаХраненияФайлов.УдалениеЛишнихФайловИзТома:
    // TextColor→Font→PictureSize; прежде слот ОТСУТСТВОВАЛ в PictureDecoration-порядке ⇒
    // Designer РОНЯЛ прочитанный `<TextColor>`).
    DesSlot::F(ld::F_TEXT_COLOR),
    // `<Font>` эмитится ПОСЛЕ геометрии, ПЕРЕД `<Title>` (по аналогии с LabelDecoration).
    DesSlot::Font,
    // shortcut — ПЕРЕД `<Title>` (witness НастройкиОбменаФСС.ФормаЗаписи ДекорацияИнформация:
    // Shortcut первым ребёнком, до Title; метамодель font→shortcut→groupHorizontalAlign).
    DesSlot::F(F_DEC_SHORTCUT),
    DesSlot::TitleFormatted,
    // toolTip — СРАЗУ после `<Title>`, ДО ToolTipRepresentation/Hyperlink/PictureSize/Picture
    // (SSL Designer PictureDecoration: Title<ToolTip×26, ToolTip<Picture×27, ToolTip<
    // ToolTipRepresentation×1, ToolTip<PictureSize×23, ToolTip<Hyperlink×17; прежде слот
    // ОТСУТСТВОВАЛ — Designer РОНЯЛ `<ToolTip>` ×27, вскрыто СертификатыКлючей.ПроверкаСертификата).
    DesSlot::F(ld::F_TOOL_TIP),
    // toolTipRepresentation — сразу после `<ToolTip>`, ПЕРЕД `<Picture>` (witness
    // ДополнительныеРеквизитыИСведения.ИзменениеНастройкиСвойства: Title→
    // ToolTipRepresentation→Picture→FileDragMode). Прежде слот отсутствовал в
    // PictureDecoration-порядке ⇒ Designer РОНЯЛ прочитанный `<ToolTipRepresentation>`.
    DesSlot::F(ld::F_TOOL_TIP_REPRESENTATION),
    // `<Zoomable>`/`<EnableStartDrag>`/`<EnableDrag>` — сразу после `<Title>` (witness Декорации).
    DesSlot::F(ld::F_EXT_ZOOMABLE),
    DesSlot::F(ld::F_EXT_ENABLE_START_DRAG),
    DesSlot::F(ld::F_EXT_ENABLE_DRAG),
    // groupHorizontalAlign — ПЕРЕД GroupVerticalAlign/Picture (SSL PictureDecoration:
    // Title<GHA×11, GHA<GVA×10, GHA<Picture×11, GHA<PictureSize×3, ToolTipRepresentation<GHA×1;
    // прежде слот ОТСУТСТВОВАЛ ⇒ Designer РОНЯЛ прочитанный <GroupHorizontalAlign>,
    // напр. НапоминанияПользователя.Настройки).
    DesSlot::F(ld::F_GROUP_HORIZONTAL_ALIGN),
    DesSlot::F(ld::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(ld::F_EXT_HYPERLINK),
    DesSlot::F(ld::F_EXT_PICTURE_SIZE),
    DesSlot::F(ld::F_EXT_IMAGE_SCALE),
    // NonselectedPictureText — ДО Picture (witness УправлениеПодключениемDSS:
    // GroupHorizontalAlign→NonselectedPictureText→Picture).
    DesSlot::F(ld::F_EXT_NONSELECTED_PICTURE_TEXT),
    DesSlot::F(ld::F_EXT_PICTURE),
    // PictureColor — ПОСЛЕ Picture (SSL Designer PictureDecoration: Picture<PictureColor×4,
    // PictureSize<PictureColor×1; 0 контрпримеров — прежняя позиция сразу после PictureSize
    // вскрыта ТомаХраненияФайлов.УдалениеЛишнихФайловИзТома).
    DesSlot::F(ld::F_EXT_PICTURE_COLOR),
    // borderColor — ПЕРЕД Border (witness Мастер_ПаспортныеДанные Силуэт: Picture→BorderColor→
    // Border; EDT-порядок обратный border→borderColor, см. PICTURE_DECORATION_EXT).
    DesSlot::F(F_LD_BORDER_COLOR),
    DesSlot::F(ld::F_EXT_PIC_BORDER),
    DesSlot::F(ld::F_EXT_FILE_DRAG_MODE),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    DesSlot::Events,
];

/// Designer-порядок эмиссии FormField (общие + extInfo INLINE; pooled topo по
/// InputField+CheckBoxField+LabelField, конфликтов 0). Поля, отсутствующие в таблице
/// конкретного типа, пропускаются lookup'ом.
pub(crate) static DES_FIELD_ORDER: &[DesSlot] = &[
    // Голова поля — topo SSL Designer (0 циклов): DataPath, Visible, UserVisible, DefaultItem,
    // Enabled, ReadOnly, SkipOnInput (DataPath<Visible×155/UserVisible×340, Visible<UserVisible×5,
    // UserVisible<DefaultItem×3). Прежде DefaultItem/UserVisible шли перед Visible — ломало
    // Visible→UserVisible (СостоянияОригиналов/УправлениеИтогами) и DefaultItem-порядок.
    DesSlot::F(ff::F_DATA_PATH),
    DesSlot::F(ff::F_VISIBLE),
    DesSlot::F(ff::F_USER_VISIBLE),
    DesSlot::F(ff::F_DEFAULT_ITEM),
    DesSlot::F(ff::F_ENABLED),
    DesSlot::F(ff::F_READ_ONLY),
    DesSlot::F(ff::F_SKIP_ON_INPUT),
    DesSlot::F(ff::F_TITLE),
    DesSlot::F(ff::F_TITLE_TEXT_COLOR),
    // TitleFont — СРАЗУ после Title/TitleTextColor (witness LabelField Title→TitleFont→EditMode ×6,
    // InputField Title→TitleFont→TitleLocation ×3; метамодель titleTextColor→titleFont).
    DesSlot::TitleFont,
    // TitleBackColor — после Title-блока, ДО EditMode (ERP-witness Мастер_Направления
    // Title→TitleBackColor→EditMode; соседи-пары с TitleLocation не витнессированы — слот
    // loose внутри интервала).
    DesSlot::F(F_FF_TITLE_BACK_COLOR),
    DesSlot::F(ff::F_TITLE_LOCATION),
    DesSlot::F(ff::F_TITLE_HEIGHT),
    DesSlot::F(ff::F_TOOL_TIP),
    DesSlot::F(ff::F_TOOL_TIP_REPRESENTATION),
    // shortcut — ПОСЛЕ ToolTip/ToolTipRepresentation (SSL Designer: ToolTip<Shortcut×1,
    // Shortcut<EditMode×6; напр. ЗадачаИсполнителя.ЗадачиПоПредмету ВажностьКартинка:
    // TitleLocation→ToolTip→Shortcut→EditMode). Прежняя позиция до ToolTip ломала её.
    DesSlot::F(ff::F_SHORTCUT),
    // horizontalAlign — ПОСЛЕ toolTip/title/titleLocation, ДО groupHorizontalAlign (SSL Designer:
    // ToolTip<HorizontalAlign×22, Title×74, TitleLocation×33, HorizontalAlign<GroupHorizontalAlign×3).
    // Прежде стоял сразу после TitleLocation — ломал ToolTip→HorizontalAlign (ТомаХраненияФайлов/
    // ЗащитаПерсональныхДанных).
    DesSlot::F(ff::F_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_GROUP_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_VERTICAL_ALIGN),
    DesSlot::F(ff::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(ff::F_WARNING_ON_EDIT_REPRESENTATION),
    DesSlot::F(ff::F_WARNING_ON_EDIT),
    DesSlot::F(ff::F_EDIT_MODE),
    // FooterDataPath/FooterTextColor — СРАЗУ после EditMode (ERP-witness ОплатыСумма:
    // EditMode→FooterDataPath→ContextMenu; РаспоряженияНаДоставкуВес:
    // EditMode→FooterTextColor→FooterFont→Width; пары с FixingInTable не витнессированы —
    // слоты loose внутри интервала EditMode..геометрия).
    DesSlot::F(F_FF_FOOTER_DATA_PATH),
    DesSlot::F(F_FF_FOOTER_TEXT_COLOR),
    DesSlot::FooterFont,
    DesSlot::F(F_FF_FOOTER_PICTURE),
    DesSlot::F(ff::F_FIXING_IN_TABLE),
    DesSlot::F(ff::F_CELL_HYPERLINK),
    DesSlot::F(ff::F_AUTO_CELL_HEIGHT),
    DesSlot::F(ff::F_FOOTER_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_HEADER_PICTURE),
    DesSlot::F(ff::F_HEADER_HORIZONTAL_ALIGN),
    // showInHeader ПЕРЕД showInFooter (SSL: H<F ×16 по всем видам полей, 0 контрпримеров;
    // напр. ШаблоныСообщений.ФормаСписка СтандартнаяКартинка). Прежний порядок был обратным.
    DesSlot::F(ff::F_SHOW_IN_HEADER),
    DesSlot::F(ff::F_SHOW_IN_FOOTER),
    // markRequiredComplete — ПЕРЕД AutoEditMode (корпус Designer: EditMode→MarkRequiredComplete×11,
    // MarkRequiredComplete→AutoEditMode×11; при отсутствии EditMode следует за WarningOnEdit).
    // FooterText — МЕЖДУ EditMode и AutoEditMode (witness СообщениеSMS ×2:
    // EditMode→FooterText→AutoEditMode; поля-соседи в витнессах опущены ⇒ слот loose
    // внутри интервала, ставим после ShowInFooter, перед MarkRequiredComplete).
    DesSlot::F(ff::F_FOOTER_TEXT),
    DesSlot::F(ff::F_MARK_REQUIRED_COMPLETE),
    DesSlot::AutoEditMode,
    // ShowTitleInCard→AutoWidthInTable — СРАЗУ после AutoEditMode, ДО CellHyperlink*
    // (SSL: AutoEditMode<AWIT×196, ShowTitleInCard<AWIT×2, AWIT<CellHyperlinkRepresentation×1
    // ПользовательскиеМакетыПечати; 0 контрпримеров — прежний хвост AWIT-после-CH* ломал её).
    DesSlot::F(ff::F_SHOW_TITLE_IN_CARD),
    DesSlot::F(F_FF_WIDTH_IN_CARD),
    DesSlot::F(ff::F_AUTO_WIDTH_IN_TABLE),
    DesSlot::F(ff::F_CELL_HYPERLINK_REPRESENTATION),
    DesSlot::F(ff::F_CELL_HYPERLINK_DISPLAY_VARIANT),
    // extInfo-поля INLINE (union top3; per-type lookup пропускает чужие).
    DesSlot::F(ff::F_EXT_CHECK_BOX_TYPE),
    DesSlot::F(ff::F_EXT_THREE_STATE),
    DesSlot::F(ff::F_EXT_WIDTH),
    DesSlot::F(ff::F_EXT_AUTO_MAX_WIDTH),
    // maxWidth — СРАЗУ после autoMaxWidth, ДО height/horizontalStretch (SSL Designer:
    // AutoMaxWidth<MaxWidth×51/Width×7, MaxWidth<HorizontalStretch×8/Height×8). Прежде стоял ПОСЛЕ
    // HorizontalStretch — ломал MaxWidth→HorizontalStretch (напр. ТранспортFTP.ФормаНастройки).
    DesSlot::F(ff::F_EXT_MAX_WIDTH),
    DesSlot::F(ff::F_EXT_HEIGHT),
    DesSlot::F(ff::F_EXT_AUTO_MAX_HEIGHT),
    DesSlot::F(ff::F_EXT_MAX_HEIGHT),
    DesSlot::F(ff::F_EXT_HORIZONTAL_STRETCH),
    DesSlot::F(ff::F_EXT_HYPERLINK),
    DesSlot::F(ff::F_EXT_VERTICAL_STRETCH),
    // PDFDocumentField scale/currentPageNumber — ПОСЛЕ геометрии (метамодель PDFDocumentFieldExtInfo:
    // …verticalStretch→scale→currentPageNumber). Designer их опускает (KEEP-дефолты 100/1 — 4/4
    // ERP-витнесса), поэтому позиция de-facto нейтральна; носитель — только PDFDocumentField
    // (у прочих видов id нет в ext-таблице ⇒ слот пропускается).
    DesSlot::F(ff::F_EXT_SCALE),
    DesSlot::F(ff::F_EXT_CURRENT_PAGE_NUMBER),
    // LabelField border — СРАЗУ после VerticalStretch, ДО BackColor (witness
    // ПоискИУдалениеДублей.ПоискДублей: VerticalStretch→Border→BackColor). Собственный id:
    // PictureField-слот Border (после ValuesPicture/цветов) с этим порядком несовместим.
    DesSlot::F(ff::F_EXT_LABEL_BORDER),
    // GraphicalSchemaField `edit` — единственный носитель F_EXT_EDIT в DES_FIELD_ORDER (Spreadsheet
    // использует DES_SPREADSHEET_ORDER); Designer эмитит `<Edit>` ПОСЛЕ Width/Height (stretch
    // дефолтны/опущены). Для прочих видов слот пуст (поля нет в bag). Witness КартаМаршрута.
    DesSlot::F(ff::F_EXT_EDIT),
    // wrap — после horizontal/verticalStretch, ПЕРЕД multiLine/spinButton (SSL Designer:
    // HorizontalStretch<Wrap×20, VerticalStretch<Wrap×3, Wrap<MultiLine×10, Wrap<SpinButton×8).
    // spinButton перенесён ПОСЛЕ wrap/multiLine (напр. УправлениеИтогами.ФормаПараметровПерестроения).
    DesSlot::F(ff::F_EXT_WRAP),
    // passwordMode — ПЕРЕД MultiLine (SSL Designer: PasswordMode<MultiLine×1, AutoMaxWidth<
    // PasswordMode×11, HorizontalStretch<PasswordMode×7; MultiLine<PasswordMode НЕ витнессирован;
    // = порядок метамодели wrap→passwordMode→multiLine. Прежний слот после DropListButton
    // ломал PasswordMode→MultiLine, напр. ХранилищеВариантовОтчетов.СохранениеВариантаОтчета).
    DesSlot::F(ff::F_EXT_PASSWORD_MODE),
    DesSlot::F(ff::F_EXT_MULTI_LINE),
    DesSlot::F(ff::F_EXT_ALLOW_INPUT_EMPTY_MULTIPLE_VALUES),
    DesSlot::F(ff::F_EXT_ALLOW_MULTIPLE_VALUES_DUPLICATES),
    DesSlot::F(ff::F_EXT_DROP_LIST_BUTTON),
    // ExtendedEdit — после MultiLine/PasswordMode, до ChoiceButton/ExtendedEditMultipleValues/
    // EditFormat/ListChoiceMode/ChooseType/TextEdit (SSL-витнессы 33 InputField'ов, конфликтов 0;
    // напр. `PasswordMode > ExtendedEdit`, `MultiLine > ExtendedEdit > ChoiceButton`).
    DesSlot::F(ff::F_EXT_EXTENDED_EDIT),
    DesSlot::F(ff::F_EXT_CHOICE_BUTTON),
    // choiceButtonPicture — ПОЗДНЕЕ поле тела InputField (Designer): после EditTextUpdate/TextEdit/
    // ChooseType/EEMV, ДО TextColor/BorderColor/HeightControlVariant/InputHint/TextSize (SSL-витнессы:
    // EditTextUpdate<CBP×5, EEMV<CBP×4, CBP<InputHint×5, CBP<TextSize×2). Слот перенесён вниз
    // (см. позицию после F_EXT_EDIT_TEXT_UPDATE) — прежняя ранняя позиция ломала
    // ChoiceButton→EEMV→ChoiceButtonPicture.
    DesSlot::F(ff::F_EXT_CHOICE_BUTTON_REPRESENTATION),
    DesSlot::F(ff::F_EXT_CLEAR_BUTTON),
    // mask — после ChoiceButton/ClearButton/MaxWidth, ДО openButton/editFormat/listChoiceMode/EEMV/
    // autoMarkIncomplete (SSL-витнессы: MaxWidth/ChoiceButton<Mask; Mask<EEMV×2, Mask<AutoMarkIncomplete×10,
    // Mask<EditFormat/AutoChoiceIncomplete/ChooseType). Перенесён вверх из позиции перед AutoMarkIncomplete —
    // там ломал Mask→EEMV (напр. Подписанты.ДобавлениеПодписанта: MaxWidth→Mask→EEMV).
    DesSlot::F(ff::F_EXT_MASK),
    // spinButton — ПОСЛЕ ChoiceButton/ClearButton/DropListButton/PasswordMode, ДО OpenButton/
    // CreateButton/EditFormat (SSL Designer: ChoiceButton<SpinButton×22, ClearButton<SpinButton×23,
    // DropListButton<SpinButton×9, PasswordMode<SpinButton×1, SpinButton<OpenButton×21,
    // SpinButton<CreateButton×9, SpinButton<EditFormat×11; 0 контрпримеров; напр.
    // ВариантыОтчетов.РазмещениеВРазделах). Прежняя позиция после MultiLine ломала
    // ClearButton→SpinButton→OpenButton.
    DesSlot::F(ff::F_EXT_SPIN_BUTTON),
    // openButton/createButton — ПЕРЕД format/editFormat/listChoiceMode (корпус SSL Designer:
    // ClearButton→OpenButton×25, ChoiceButton→OpenButton×34, OpenButton→CreateButton×17,
    // OpenButton→ListChoiceMode×9; метамодель openButton 29 < format 35 < listChoiceMode 37).
    DesSlot::F(ff::F_EXT_OPEN_BUTTON),
    DesSlot::F(ff::F_EXT_CREATE_BUTTON),
    // ListChoiceMode — ДО Format/EditFormat (SSL Designer: ListChoiceMode<EditFormat×4,
    // <Format×1; DropListButton/ChoiceButton/OpenButton/SpinButton/CreateButton<ListChoiceMode;
    // 0 контрпримеров — прежняя позиция ПОСЛЕ EditFormat вскрыта
    // СопоставлениеОбъектовИнформационныхБаз.НастройкаСортировки).
    DesSlot::F(ff::F_EXT_LIST_CHOICE_MODE),
    // extendedEditMultipleValues — НЕПОСРЕДСТВЕННО после listChoiceMode, ДО Format/EditFormat
    // (SSL-витнессы: ListChoiceMode→EEMV×24, EEMV→EditFormat×1 — РегистрацияИзменений
    // ДляОбменаДанными; 0 контрпримеров — прежняя позиция ПОСЛЕ EditFormat ломала её).
    // Остаётся ДО MultipleValueDataPath/AutoMarkIncomplete/ChooseType/TextEdit (все SSL-витнессы
    // EEMV<X); Mask же идёт РАНЬШЕ (Mask<EEMV — см. слот mask выше, после ClearButton).
    DesSlot::F(ff::F_EXT_EXTENDED_EDIT_MULTIPLE_VALUES),
    // ShowCheckBoxesInDropList/MarkNegatives — после EEMV (ERP-witness Ozon:
    // ListChoiceMode→ShowCheckBoxesInDropList; списковые колонки: DataPath→MarkNegatives→
    // ContextMenu; взаимный порядок пары не витнессирован — метамодельный #17→#18).
    DesSlot::F(F_EXT_SHOW_CHECK_BOXES_IN_DROP_LIST),
    DesSlot::F(F_EXT_MARK_NEGATIVES),
    DesSlot::F(ff::F_EXT_FORMAT),
    DesSlot::F(ff::F_EXT_EDIT_FORMAT),
    DesSlot::F(ff::F_EXT_AUTO_CHOICE_INCOMPLETE),
    DesSlot::F(ff::F_EXT_MULTIPLE_VALUE_DATA_PATH),
    // QuickChoice → ChoiceFoldersAndItems → AutoMarkIncomplete → ChooseType (SSL Designer:
    // QuickChoice<CFAI×1, CFAI<AutoMarkIncomplete×2, AutoMarkIncomplete<ChooseType×1,
    // QuickChoice<AutoMarkIncomplete×1 — ЕДИНСТВЕННЫЙ корпусный QC/AMI-witness; 0 контрпримеров.
    // Прежний порядок AMI→QC и CFAI-в-хвосте вскрыты ШаблоныАнкет.ФормаПростыхВопросов).
    DesSlot::F(ff::F_EXT_QUICK_CHOICE),
    DesSlot::F(ff::F_EXT_CHOICE_FOLDERS_AND_ITEMS),
    DesSlot::F(ff::F_EXT_AUTO_MARK_INCOMPLETE),
    DesSlot::F(ff::F_EXT_CHOOSE_TYPE),
    // incompleteChoiceMode — ПОСЛЕ AutoMarkIncomplete/ChooseType/QuickChoice, ДО TextEdit/
    // EditTextUpdate/ChoiceParameters/ChoiceList (SSL Designer: AutoMarkIncomplete<ICM×2,
    // ChooseType<ICM×9, QuickChoice<ICM×2, ICM<TextEdit×6, ICM<ChoiceList×3, ICM<BackColor×1;
    // 0 контрпримеров — прежняя позиция до MinValue ломала AutoMarkIncomplete→ChooseType→ICM,
    // напр. УдалениеПомеченныхОбъектов.ОсновнаяФорма).
    DesSlot::F(ff::F_EXT_INCOMPLETE_CHOICE_MODE),
    DesSlot::F(ff::F_EXT_TYPE_DOMAIN_ENABLED),
    // MinValue/MaxValue — ПОСЛЕ AutoMarkIncomplete/ChooseType/TypeDomainEnabled, ДО ChoiceList
    // (SSL Designer: AMI<MinValue×2 Календари, ChooseType<MinValue×2, TypeDomainEnabled<
    // MinValue×2 ЖурналРегистрации/ЗащитаПД, MinValue<ChoiceList×2; 0 контрпримеров —
    // прежняя ранняя позиция до MVDP/QuickChoice ломала все три).
    DesSlot::F(ff::F_EXT_MIN_VALUE),
    DesSlot::F(ff::F_EXT_MAX_VALUE),
    DesSlot::F(ff::F_EXT_TEXT_EDIT),
    // editTextUpdate — СРАЗУ после TextEdit, ДО ChoiceParameters/ChoiceList/ChoiceButtonPicture
    // (SSL Designer: TextEdit<ETU×3, ListChoiceMode<ETU×3, IncompleteChoiceMode<ETU×1,
    // ETU<ChoiceList×3, ETU<ChoiceButtonPicture×5, ETU<BorderColor×1, ETU<ChoiceListHeight×1;
    // 0 контрпримеров — прежняя позиция ПОСЛЕ BackColor/ChoiceList ломала ETU→ChoiceList,
    // напр. УдалениеПомеченныхОбъектов.ОсновнаяФорма).
    DesSlot::F(ff::F_EXT_EDIT_TEXT_UPDATE),
    // ChoiceForm/ChoiceParameterLinks — ДО ChoiceParameters (метамодель 47→48→49; Designer-
    // витнессы: OpenButton→ChoiceForm→ContextMenu; TextEdit/ChooseType→ChoiceParameterLinks→
    // ChoiceParameters, AutoMarkIncomplete→CPL→ContextMenu; 0 контрпримеров).
    DesSlot::F(ff::F_EXT_CHOICE_FORM),
    DesSlot::F(ff::F_EXT_CHOICE_PARAMETER_LINKS),
    // InputField choiceParameters — ПОСЛЕ textEdit, ДО choiceList (witness ОтправкаСообщения:
    // ChoiceParameters сразу после AutoEditMode, промежуточные ext-поля отсутствуют ⇒ слот loose).
    DesSlot::F(ff::F_EXT_CHOICE_PARAMETERS),
    // InputField availableTypes — ПОСЛЕ choiceParameters (зеркало EDT-метамодели:
    // choiceParameters 49 → availableTypes 50), ДО InputHint/ContextMenu. Designer-витнессы SSL
    // (5 форм): после Wrap/DropListButton (ФормаПоиска, Поиск), ChooseType (Заявление…),
    // TypeDomainEnabled (РегламентноеЗадание, Валюты); всегда до
    // InputHint/ChoiceHistoryOnInput/ContextMenu. Конфликтов 0.
    DesSlot::F(ff::F_EXT_AVAILABLE_TYPES),
    // InputField choiceList — ПОСЛЕ textEdit, ДО backColor (сверено).
    DesSlot::F(ff::F_EXT_CHOICE_LIST),
    // choiceListButton ПЕРЕД backColor (SSL Designer: AutoEditMode/TextEdit<ChoiceListButton,
    // ChoiceListButton<BackColor; напр. ПрограммыЭлектроннойПодписиИШифрования.ФормаЭлемента).
    DesSlot::F(ff::F_EXT_CHOICE_LIST_BUTTON),
    // choiceListHeight — ПОЗДНИЙ слот: ПОСЛЕ ChoiceList/EditTextUpdate/TextEdit, ДО
    // DropListWidth/ContextMenu (SSL Designer: ChoiceList<CLH×2, EditTextUpdate<CLH×1,
    // TextEdit<CLH×2, ListChoiceMode<CLH×2, CLH<DropListWidth×2; 0 контрпримеров — прежняя
    // ранняя позиция до MinValue ломала ChoiceList→CLH).
    DesSlot::F(ff::F_EXT_CHOICE_LIST_HEIGHT),
    // DropListWidth — ПОСЛЕ ChoiceListHeight, ДО BackColor (SSL Designer:
    // ChoiceListHeight<DLW×2, DLW<BackColor×1, 0 контрпримеров — прежняя позиция после
    // Spell* вскрыта ВопросыДляАнкетирования.ФормаЭлемента).
    DesSlot::F(ff::F_EXT_DROP_LIST_WIDTH),
    DesSlot::F(ff::F_EXT_BACK_COLOR),
    // choiceButtonPicture занимает ЭТУ позицию (после EditTextUpdate/BackColor, до TextColor) —
    // перенесён из ранней позиции choiceButton-семейства (witness: EditTextUpdate<CBP,
    // CBP<TextColor/InputHint).
    DesSlot::F(ff::F_EXT_CHOICE_BUTTON_PICTURE),
    DesSlot::F(F_EXT_CHOICE_BUTTON_TITLE),
    DesSlot::F(ff::F_EXT_TEXT_COLOR),
    // BorderColor — после цветов/ChoiceList/EditTextUpdate/ListChoiceMode, до
    // HeightControlVariant/ContextMenu (SSL-витнессы 42 InputField'ов; напр.
    // `BackColor > BorderColor`, `TextColor > BorderColor`, `BorderColor > HeightControlVariant`).
    DesSlot::F(ff::F_EXT_BORDER_COLOR),
    // HeightControlVariant — ЗАМЫКАЮЩЕЕ ext-свойство (все 16 SSL/coverage-витнессов несут его
    // НЕПОСРЕДСТВЕННО перед ContextMenu: после ClearButton/TextEdit/MultipleValueDataPath/
    // ChoiceButtonPicture/OpenButton/BorderColor; прежняя позиция до OpenButton противоречила
    // витнессам `ChoiceButton > OpenButton > TextEdit > HeightControlVariant`).
    DesSlot::F(ff::F_EXT_HEIGHT_CONTROL_VARIANT),
    DesSlot::F(ff::F_EXT_AUTO_SHOW_OPEN_BUTTON_MODE),
    DesSlot::F(ff::F_EXT_AUTO_SHOW_CLEAR_BUTTON_MODE),
    DesSlot::F(ff::F_EXT_AUTO_CORRECTION_ON_TEXT_INPUT),
    DesSlot::F(ff::F_EXT_SPELL_CHECKING_ON_TEXT_INPUT),
    // SpecialTextInputMode — ПОСЛЕ EEMV-кластера, ДО InputHint (ERP-witness МЧД003:
    // ExtendedEditMultipleValues→SpecialTextInputMode→ContextMenu; loose).
    DesSlot::F(F_EXT_SPECIAL_TEXT_INPUT_MODE),
    DesSlot::F(ff::F_EXT_INPUT_HINT),
    // picture (InputField-поиск) — ПОСЛЕ InputHint (SSL Designer: InputHint<Picture×2,
    // ClearButton<Picture×3, SpinButton<Picture×3, OpenButton<Picture×1, ChoiceButton<Picture×1;
    // 0 контрпримеров; напр. НаборыДополнительныхРеквизитовИСведений.ФормаСписка ПолеПоиска).
    // Прежняя позиция сразу после ChoiceButton ломала ClearButton→InputHint→Picture.
    DesSlot::F(ff::F_EXT_PICTURE),
    // LabelField `<Font>` — ПЕРЕД UseCopy (witness VerticalStretch→Font→UseCopy;
    // Hiperlink/Height→Font→ContextMenu; метамодель LabelFieldExtInfo backColor→font→useCopy).
    DesSlot::Font,
    DesSlot::F(ff::F_EXT_USE_COPY),
    // textSize — ЗАМЫКАЮЩЕЕ поле тела InputField в Designer, НЕПОСРЕДСТВЕННО перед ContextMenu
    // (witness SSL: InputHint→TextSize→ContextMenu; 82 экземпляра, все `Enlarged`). Опускается при
    // дефолте `Normal` (KEEP Eq). В EDT — extInfo-поле (Region::Ext), в Designer — тело контрола.
    DesSlot::F(ff::F_EXT_TEXT_SIZE),
    DesSlot::F(F_EXT_DROP_LIST_HINT),
    // RadioButtonField extInfo (ids 201+ — не пересекаются с InputField 101+; для прочих
    // типов lookup не находит их и пропускает). Порядок = Designer topo: RadioButtonType,
    // Orientation, ColumnsCount, ChoiceList; ChoiceList — последний ext ДО декораторов.
    DesSlot::F(rb::F_EXT_RADIO_BUTTON_TYPE),
    // ItemWidth — после RadioButtonType, ДО ItemHeight (ERP-witness
    // НастройкиРасчетаРезервовПоОплатеТруда RadioButtonType→ItemWidth→EqualColumnsWidth;
    // общий слот CheckBox/Radio).
    DesSlot::F(F_EXT_ITEM_WIDTH),
    DesSlot::F(rb::F_EXT_ITEM_HEIGHT),
    DesSlot::F(rb::F_EXT_ITEM_TITLE_HEIGHT),
    DesSlot::F(rb::F_EXT_ORIENTATION),
    DesSlot::F(rb::F_EXT_COLUMNS_COUNT),
    // EqualColumnsWidth — после RadioButtonType/ColumnsCount, ДО ChoiceList (witness
    // ПомощникСозданияОбменаДанными: RadioButtonType→EqualColumnsWidth→ChoiceList).
    DesSlot::F(rb::F_EXT_EQUAL_ELEMENTS_WIDTH),
    // horizontalStretch ПЕРЕД choiceList (SSL Designer RadioButtonField:
    // HorizontalStretch<ChoiceList×9; напр. НастройкиАрхиваСообщенийОбменов.ФормаЗаписи).
    DesSlot::F(rb::F_EXT_HORIZONTAL_STRETCH),
    DesSlot::F(rb::F_EXT_CHOICE_LIST),
    // document/picture FormField-подтипы (LANE-F-5). Их геометрия Designer ОПУСКАЕТ (HTML/
    // Progress/Formatted) либо эмитит на общих слотах выше (PictureField Symmetric); здесь —
    // ТОЛЬКО Designer-эмитимые тип-специфичные поля, в EDT-порядке. Для одиночных типов
    // (output/showPercent) позиция нерелевантна (иных ext-полей тип не эмитит); блок
    // PictureField (pictureSize→fileDragMode) идёт смежно, ПОСЛЕ геометрии.
    DesSlot::F(ff::F_EXT_OUTPUT),
    DesSlot::F(ff::F_EXT_SHOW_PERCENT),
    // TrackBar/ProgressBar тип-специфичные (Designer эмитит не-дефолт; позиция loose — каждый
    // коврол покрытия несёт одно свойство).
    DesSlot::F(ff::F_EXT_STEP),
    DesSlot::F(ff::F_EXT_LARGE_STEP),
    DesSlot::F(ff::F_EXT_MARKING_STEP),
    DesSlot::F(ff::F_EXT_MARKING_APPEARANCE),
    DesSlot::F(ff::F_EXT_ORIENTATION),
    DesSlot::F(ff::F_EXT_PROGRESS_REPRESENTATION),
    DesSlot::F(ff::F_EXT_PICTURE_SIZE),
    DesSlot::F(ff::F_EXT_PIC_HYPERLINK),
    DesSlot::F(ff::F_EXT_NONSELECTED_PICTURE_TEXT),
    DesSlot::F(ff::F_EXT_VALUES_PICTURE),
    // borderColor — СРАЗУ за ValuesPicture, ДО Border (witness Новости.ФормаНовости:
    // ValuesPicture→BorderColor→Border; ERP-волна). Designer-порядок ОБРАТЕН EDT
    // (`border`→`borderColor`); отсюда и собственный id [`F_PIC_BORDER_COLOR`]. Относительно
    // PicTextColor витнесс не задан (не сходятся на одном PictureField) — кладём сразу за
    // ValuesPicture, что воспроизводит наблюдаемый ValuesPicture→BorderColor→Border.
    DesSlot::F(F_PIC_BORDER_COLOR),
    // ImageField хвост — topo SSL Designer: ValuesPicture<TextColor×1/Border×18,
    // TextColor<FileDragMode×1/PictureColor×1, Border<FileDragMode×15,
    // FileDragMode<PictureColor×1; 0 контрпримеров ⇒ TextColor→Border→FileDragMode→
    // PictureColor (прежний PictureColor-до-Border/FileDragMode вскрыт
    // Взаимодействия.ФормаСписка).
    DesSlot::F(ff::F_EXT_PIC_TEXT_COLOR),
    DesSlot::F(ff::F_EXT_BORDER),
    DesSlot::F(ff::F_EXT_FILE_DRAG_MODE),
    DesSlot::F(ff::F_EXT_PICTURE_COLOR),
    // TypeLink — поздний слот перед ChoiceHistoryOnInput/ContextMenu (ВСЕ 8 Designer-витнессов:
    // EditMode/AutoEditMode/ChooseType/AutoMaxWidth→TypeLink→ContextMenu; пара с
    // ChoiceHistoryOnInput не витнессирована — tie-break по метамодели typeLink 63 < 84).
    DesSlot::F(ff::F_EXT_TYPE_LINK),
    // InputField choiceHistoryOnInput — ЗАМЫКАЮЩЕЕ свойство Designer-эмиссии: во ВСЕХ 18
    // SSL-витнессах InputField'ов идёт НЕПОСРЕДСТВЕННО перед ContextMenu (после
    // InputHint/QuickChoice/AutoMarkIncomplete/AvailableTypes/…; контр-примеров 0 —
    // прежняя позиция до heightControlVariant противоречила 10× OpenButton→CHOI).
    DesSlot::F(ff::F_EXT_CHOICE_HISTORY_ON_INPUT),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    // PDFDocumentField `<ViewStatusAddition>` и GanttChartField `<Table>` (авто-таблица) —
    // ПОСЛЕ ExtendedTooltip, ДО Events (witness СервисДоставки.ПросмотрPDF /
    // ДиспетчированиеГрафикаПроизводства.Планирование). No-op у прочих видов (additions пусты,
    // auto_table=None). Пара additions/autoTable ни на одном контроле не со-встречается ⇒ их
    // взаимный порядок нейтрален.
    DesSlot::TableAdditions,
    DesSlot::AutoTable,
    DesSlot::Events,
];

/// Designer-порядок эмиссии семейства FormGroup (pooled topo, конфликтов 0).
pub(crate) static DES_GROUP_ORDER: &[DesSlot] = &[
    // Голова: UserVisible→Visible→Enabled→ReadOnly→EnableContentChange (SSL: UserVisible<
    // ReadOnly×1 — ВариантыОтчетов.ФормаЭлемента Page; Visible<Enabled×2; ReadOnly<
    // EnableContentChange×1; 0 контрпримеров — прежняя голова ReadOnly-первым ломала
    // UserVisible→ReadOnly).
    // Visible ПЕРЕД UserVisible (единственный ко-витнесс: ЗащитаПерсональныхДанных
    // ColumnGroup Visible→UserVisible; прежний порядок был tie-break-слепым).
    DesSlot::F(fg::F_VISIBLE),
    DesSlot::F(fg::F_USER_VISIBLE),
    DesSlot::F(fg::F_ENABLED),
    DesSlot::F(fg::F_READ_ONLY),
    DesSlot::F(fg::F_ENABLE_CONTENT_CHANGE),
    DesSlot::F(fg::F_TITLE),
    DesSlot::F(fg::F_TITLE_TEXT_COLOR),
    // TitleFont — СРАЗУ после Title (witness UsualGroup Title→TitleFont→Group ×4;
    // метамодель titleTextColor→titleFont).
    DesSlot::TitleFont,
    DesSlot::F(fg::F_SHORTCUT),
    DesSlot::F(fg::F_TOOL_TIP),
    DesSlot::F(fg::F_TOOL_TIP_REPRESENTATION),
    DesSlot::F(fg::F_WIDTH),
    DesSlot::F(fg::F_HEIGHT),
    DesSlot::F(fg::F_HORIZONTAL_STRETCH),
    DesSlot::F(fg::F_VERTICAL_STRETCH),
    // Picture: ПОСЛЕ геометрии (Popup СостоянияОригиналов: HorizontalStretch→Picture→
    // CommandSource/Importance ×1), ДО Group (Page Picture<Group×23); Page несёт его сразу
    // после Title (промежуточные слоты пусты — вывод не меняется), Popup — после ToolTip.
    DesSlot::F(fg::F_EXT_PICTURE),
    DesSlot::F(fg::F_GROUP_HORIZONTAL_ALIGN),
    DesSlot::F(fg::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(fg::F_EXT_GROUP),
    // childrenAlign — ПОСЛЕ Group, ДО HorizontalSpacing (SSL Designer: Group<ChildrenAlign×3,
    // HorizontalStretch<ChildrenAlign×1, ChildrenAlign<HorizontalSpacing×1/ShowTitle×2/
    // Representation×3/Behavior×3; 0 контрпримеров — прежняя позиция сразу после TitleTextColor
    // ломала HorizontalStretch→Group→ChildrenAlign, напр. РасширенныйВводКонтактнойИнформации).
    DesSlot::F(fg::F_EXT_CHILDREN_ALIGN),
    DesSlot::F(fg::F_EXT_HORIZONTAL_SPACING),
    DesSlot::F(fg::F_EXT_PAGES_REPRESENTATION),
    // Порядок align/spacing = метамодель (VerticalSpacing → HorizontalAlign → VerticalAlign):
    // SSL Designer HorizontalSpacing<VerticalSpacing×12, VerticalSpacing<HorizontalAlign×12,
    // HorizontalAlign<VerticalAlign×8, 0 контрпримеров (прежний VerticalAlign-первым —
    // tie-break-артефакт, вскрыт УправлениеПодключениемDSS.ВебАутентификация Page).
    DesSlot::F(fg::F_EXT_VERTICAL_SPACING),
    DesSlot::F(fg::F_EXT_HORIZONTAL_ALIGN),
    DesSlot::F(fg::F_EXT_VERTICAL_ALIGN),
    DesSlot::F(fg::F_EXT_APPEARANCE_MODE),
    // commandSource — ПОЗДНИЙ слот (SSL: ToolTip<CommandSource×4, ToolTipRepresentation<
    // CS×2, Picture<CS×1, HorizontalLocation<CS×2, AppearanceMode<CS×1; CS<Representation×2,
    // CS<ExtendedTooltip; напр. УчетныеЗаписи….ПомощникНастройки КоманднаяПанель). Прежняя
    // позиция после ChildrenAlign ломала HorizontalLocation→CommandSource.
    DesSlot::F(fg::F_EXT_COMMAND_SOURCE),
    DesSlot::F(fg::F_EXT_BEHAVIOR),
    DesSlot::F(fg::F_EXT_COLLAPSED_REPRESENTATION_TITLE),
    DesSlot::F(fg::F_EXT_COLLAPSED),
    DesSlot::F(fg::F_EXT_CONTROL_REPRESENTATION),
    DesSlot::F(fg::F_EXT_REPRESENTATION),
    // Format — после Representation, ДО ShowTitle (witness ЗагрузкаКурсовВалют ×2:
    // Group→Behavior→Representation→Format→[ShowTitle]→ExtendedTooltip).
    DesSlot::F(fg::F_EXT_FORMAT),
    DesSlot::F(fg::F_EXT_SHAPE),
    DesSlot::F(fg::F_EXT_SHAPE_REPRESENTATION),
    DesSlot::F(fg::F_EXT_SHOW_LEFT_MARGIN),
    DesSlot::F(fg::F_EXT_UNITED),
    DesSlot::F(fg::F_EXT_SLAVE_ITEMS_WIDTH),
    DesSlot::F(fg::F_EXT_SHOW_TITLE),
    // HiddenStateTitleBackColor — после ShowTitle (ERP-witness ВзаимодействиеМПЭПД:
    // ShowTitle→HiddenStateTitleBackColor→ExtendedTooltip).
    DesSlot::F(F_GRP_HIDDEN_STATE_TITLE_BACK_COLOR),
    // Hyperlink — ПОСЛЕ ShowTitle (SSL: ShowTitle<Hyperlink×1 ПравилаДляОбменаДанными,
    // Behavior<Hyperlink×1, Collapsed<Hyperlink×1; 0 контрпримеров — прежняя позиция
    // после Format ломала ShowTitle→Hyperlink).
    DesSlot::F(fg::F_EXT_HYPERLINK),
    // ColumnGroup-специфичные ext-поля (прочие группы их не несут — слот пропускается).
    DesSlot::F(fg::F_EXT_SHOW_IN_CARD),
    DesSlot::F(fg::F_EXT_SHOW_IN_HEADER),
    // HeaderHorizontalAlign/FixingInTable — после ShowInHeader (ERP-witness
    // РетроБонусыКлиентов: ShowInHeader→HeaderHorizontalAlign→ExtendedTooltip).
    DesSlot::F(ff::F_HEADER_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_FIXING_IN_TABLE),
    DesSlot::F(fg::F_EXT_HEADER_PICTURE),
    DesSlot::F(fg::F_EXT_SHOW_TITLE_IN_CARD),
    DesSlot::F(fg::F_EXT_BACK_COLOR),
    DesSlot::F(fg::F_EXT_SHOW_AS_CARD),
    DesSlot::F(fg::F_EXT_THROUGH_ALIGN),
    DesSlot::F(fg::F_EXT_TITLE_DATA_PATH),
    DesSlot::F(fg::F_EXT_SCROLL_ON_COMPRESS),
    DesSlot::F(fg::F_EXT_CURRENT_ROW_USE),
    // Popup importance — ЗАМЫКАЮЩЕЕ поле, непосредственно перед ExtendedTooltip (SSL ×4:
    // Title→Importance→ExtendedTooltip ×3, Picture→Importance→ExtendedTooltip ×1).
    DesSlot::F(fg::F_EXT_IMPORTANCE),
    // Popup BorderColor — хвост (loose; BackColor/Format реюзают существующие слоты).
    DesSlot::F(F_GRP_POPUP_BORDER_COLOR),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    DesSlot::Events,
    DesSlot::ChildItems,
];

/// Designer-порядок эмиссии SpreadsheetDocumentField (LANE-F-6). Собственный (не пуловый):
/// EditMode/AutoEditMode идут СРАЗУ после TitleLocation/CommandSet (не в позиции пула), а
/// SpreadSheet-extInfo-поля инлайн после них. Верифицирован byte-exact по 7 формам (12
/// экземпляров). Никогда-эмитимые Designer'ом поля (`PointerType`/`DrawingSelectionShowMode`/
/// `ShowGroups`) держатся в порядке для полноты — они пропускаются (DesOmit::Always).
pub(crate) static DES_SPREADSHEET_ORDER: &[DesSlot] = &[
    DesSlot::F(ff::F_DATA_PATH),
    DesSlot::F(ff::F_DEFAULT_ITEM),
    DesSlot::F(ff::F_USER_VISIBLE),
    DesSlot::F(ff::F_VISIBLE),
    DesSlot::F(ff::F_ENABLED),
    DesSlot::F(ff::F_READ_ONLY),
    DesSlot::F(ff::F_SKIP_ON_INPUT),
    DesSlot::F(ff::F_TITLE),
    // TitleFont — за Title (позиция пулового порядка; Spreadsheet-витнесса нет — слот
    // здесь ради тотальности write: read принимает TitleFont у всех видов полей).
    DesSlot::TitleFont,
    // TitleBackColor — общий body-слот (тотальность write; Spreadsheet-витнесса нет).
    DesSlot::F(F_FF_TITLE_BACK_COLOR),
    DesSlot::F(ff::F_TITLE_LOCATION),
    DesSlot::TableCommandSet,
    DesSlot::F(ff::F_EDIT_MODE),
    // FooterDataPath/FooterTextColor — общие body-слоты после EditMode (тотальность write).
    DesSlot::F(F_FF_FOOTER_DATA_PATH),
    DesSlot::F(F_FF_FOOTER_TEXT_COLOR),
    DesSlot::FooterFont,
    DesSlot::F(F_FF_FOOTER_PICTURE),
    // footerHorizontalAlign — общий body-field, эмитится между EditMode и AutoEditMode (witness
    // ПереносФайлов.ФормаОтчета SpreadsheetDocumentField); прежде отсутствовал в порядке ⇒ Designer
    // РОНЯЛ его (roundtrip-баг, вскрытый по мере чтения формы).
    DesSlot::F(ff::F_FOOTER_HORIZONTAL_ALIGN),
    DesSlot::AutoEditMode,
    DesSlot::F(F_FF_WIDTH_IN_CARD),
    DesSlot::F(ff::F_EXT_WIDTH),
    DesSlot::F(ff::F_EXT_AUTO_MAX_WIDTH),
    // maxWidth — общий geo-слот (ERP-witness; Spreadsheet designer-порядок loose).
    DesSlot::F(ff::F_EXT_MAX_WIDTH),
    DesSlot::F(ff::F_EXT_HEIGHT),
    DesSlot::F(ff::F_EXT_AUTO_MAX_HEIGHT),
    DesSlot::F(ff::F_EXT_MAX_HEIGHT),
    DesSlot::F(ff::F_EXT_HORIZONTAL_STRETCH),
    DesSlot::F(ff::F_EXT_VERTICAL_STRETCH),
    DesSlot::F(ff::F_EXT_SHOW_GRID),
    DesSlot::F(ff::F_EXT_SHOW_HEADERS),
    DesSlot::F(ff::F_EXT_SHOW_CELL_NAMES),
    DesSlot::F(ff::F_EXT_SHOW_ROW_AND_COLUMN_NAMES),
    DesSlot::F(ff::F_EXT_POINTER_TYPE),
    DesSlot::F(ff::F_EXT_VERTICAL_SCROLL_BAR),
    DesSlot::F(ff::F_EXT_HORIZONTAL_SCROLL_BAR),
    // ViewScalingMode — после HorizontalScrollBar (ERP-witness ГосударственныеКонтракты:
    // HorizontalScrollBar→ViewScalingMode→ContextMenu).
    DesSlot::F(F_EXT_VIEW_SCALING_MODE),
    // Protection — после скроллбаров/AutoEditMode, ДО SelectionShowMode/ContextMenu (witness
    // СнимкиОтчетов: AutoEditMode→Protection→ContextMenu; VerticalScrollBar→HorizontalScrollBar→Protection).
    DesSlot::F(ff::F_EXT_PROTECTION),
    DesSlot::F(ff::F_EXT_SELECTION_SHOW_MODE),
    DesSlot::F(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE),
    DesSlot::F(ff::F_EXT_OUTPUT),
    DesSlot::F(ff::F_EXT_EDIT),
    DesSlot::F(ff::F_EXT_SHOW_GROUPS),
    DesSlot::F(ff::F_EXT_ENABLE_START_DRAG),
    DesSlot::F(ff::F_EXT_ENABLE_DRAG),
    // BorderColor — последнее поле перед ContextMenu (witness ПодтверждениеПользователя:
    // SelectionShowMode→BorderColor→ContextMenu).
    DesSlot::F(ff::F_EXT_BORDER_COLOR),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    DesSlot::Events,
];

/// Designer-порядок эмиссии CalendarField (LANE-F-7). Общий FormField-префикс (как пуловый)
/// плюс CalendarField-extInfo инлайн после общих полей; верифицирован byte-exact по форме
/// `ВыборДаты`. Designer-эмитимые ext-поля — `Width`/`Height`/`ShowCurrentDate`/`WidthInMonths`
/// (в корпусной последовательности); никогда-эмитимые (`autoMax*`, `stretch`,
/// `calendarNavigation`, `border`, `heightInMonths`) пропускаются lookup'ом или политикой.
pub(crate) static DES_CALENDAR_ORDER: &[DesSlot] = &[
    DesSlot::F(ff::F_DATA_PATH),
    DesSlot::F(ff::F_DEFAULT_ITEM),
    DesSlot::F(ff::F_USER_VISIBLE),
    DesSlot::F(ff::F_VISIBLE),
    DesSlot::F(ff::F_ENABLED),
    DesSlot::F(ff::F_READ_ONLY),
    DesSlot::F(ff::F_SKIP_ON_INPUT),
    DesSlot::F(ff::F_TITLE),
    DesSlot::F(ff::F_TITLE_TEXT_COLOR),
    // TitleFont — СРАЗУ после Title/TitleTextColor (witness LabelField Title→TitleFont→EditMode ×6,
    // InputField Title→TitleFont→TitleLocation ×3; метамодель titleTextColor→titleFont).
    DesSlot::TitleFont,
    // TitleBackColor — общий body-слот (тотальность write; Calendar-витнесса нет).
    DesSlot::F(F_FF_TITLE_BACK_COLOR),
    DesSlot::F(ff::F_TITLE_LOCATION),
    DesSlot::F(ff::F_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_TITLE_HEIGHT),
    DesSlot::F(ff::F_SHORTCUT),
    DesSlot::F(ff::F_TOOL_TIP),
    DesSlot::F(ff::F_TOOL_TIP_REPRESENTATION),
    DesSlot::F(ff::F_GROUP_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_VERTICAL_ALIGN),
    DesSlot::F(ff::F_GROUP_VERTICAL_ALIGN),
    DesSlot::F(ff::F_WARNING_ON_EDIT_REPRESENTATION),
    DesSlot::F(ff::F_WARNING_ON_EDIT),
    DesSlot::F(ff::F_EDIT_MODE),
    // FooterDataPath/FooterTextColor — СРАЗУ после EditMode (ERP-witness ОплатыСумма:
    // EditMode→FooterDataPath→ContextMenu; РаспоряженияНаДоставкуВес:
    // EditMode→FooterTextColor→FooterFont→Width; пары с FixingInTable не витнессированы —
    // слоты loose внутри интервала EditMode..геометрия).
    DesSlot::F(F_FF_FOOTER_DATA_PATH),
    DesSlot::F(F_FF_FOOTER_TEXT_COLOR),
    DesSlot::FooterFont,
    DesSlot::F(F_FF_FOOTER_PICTURE),
    DesSlot::F(ff::F_FIXING_IN_TABLE),
    DesSlot::F(ff::F_CELL_HYPERLINK),
    DesSlot::F(ff::F_AUTO_CELL_HEIGHT),
    DesSlot::F(ff::F_FOOTER_HORIZONTAL_ALIGN),
    DesSlot::F(ff::F_HEADER_PICTURE),
    DesSlot::F(ff::F_HEADER_HORIZONTAL_ALIGN),
    // showInHeader ПЕРЕД showInFooter (SSL: H<F ×16 по всем видам полей, 0 контрпримеров;
    // напр. ШаблоныСообщений.ФормаСписка СтандартнаяКартинка). Прежний порядок был обратным.
    DesSlot::F(ff::F_SHOW_IN_HEADER),
    DesSlot::F(ff::F_SHOW_IN_FOOTER),
    // markRequiredComplete — ПЕРЕД AutoEditMode (корпус Designer: EditMode→MarkRequiredComplete×11,
    // MarkRequiredComplete→AutoEditMode×11; при отсутствии EditMode следует за WarningOnEdit).
    // FooterText — общий body-слот (как в пуловом DES_FIELD_ORDER; на Calendar не витнессирован,
    // lookup пропускает при отсутствии).
    DesSlot::F(ff::F_FOOTER_TEXT),
    DesSlot::F(ff::F_MARK_REQUIRED_COMPLETE),
    DesSlot::AutoEditMode,
    // ShowTitleInCard→AutoWidthInTable — СРАЗУ после AutoEditMode, ДО CellHyperlink*
    // (SSL: AutoEditMode<AWIT×196, ShowTitleInCard<AWIT×2, AWIT<CellHyperlinkRepresentation×1
    // ПользовательскиеМакетыПечати; 0 контрпримеров — прежний хвост AWIT-после-CH* ломал её).
    DesSlot::F(ff::F_SHOW_TITLE_IN_CARD),
    DesSlot::F(F_FF_WIDTH_IN_CARD),
    DesSlot::F(ff::F_AUTO_WIDTH_IN_TABLE),
    DesSlot::F(ff::F_CELL_HYPERLINK_REPRESENTATION),
    DesSlot::F(ff::F_CELL_HYPERLINK_DISPLAY_VARIANT),
    // extInfo CalendarField INLINE (эмитимые в корпусной последовательности).
    DesSlot::F(ff::F_EXT_WIDTH),
    DesSlot::F(ff::F_EXT_AUTO_MAX_WIDTH),
    DesSlot::F(ff::F_EXT_HEIGHT),
    DesSlot::F(ff::F_EXT_AUTO_MAX_HEIGHT),
    DesSlot::F(ff::F_EXT_CALENDAR_SELECTION_MODE),
    DesSlot::F(ff::F_EXT_SHOW_CURRENT_DATE),
    DesSlot::F(ff::F_EXT_HORIZONTAL_STRETCH),
    DesSlot::F(ff::F_EXT_VERTICAL_STRETCH),
    DesSlot::F(ff::F_EXT_CALENDAR_NAVIGATION),
    DesSlot::F(ff::F_EXT_ENABLE_START_DRAG),
    DesSlot::F(ff::F_EXT_ENABLE_DRAG),
    DesSlot::F(ff::F_EXT_SHOW_MONTHS_PANEL),
    DesSlot::F(ff::F_EXT_WIDTH_IN_MONTHS),
    DesSlot::F(ff::F_EXT_HEIGHT_IN_MONTHS),
    DesSlot::F(ff::F_EXT_BORDER),
    DesSlot::ContextMenu,
    DesSlot::ExtendedTooltip,
    DesSlot::Events,
];
