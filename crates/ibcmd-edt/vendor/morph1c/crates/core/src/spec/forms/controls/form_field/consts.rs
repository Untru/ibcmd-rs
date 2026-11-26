use super::*;

// ============================ общие свойства поля ============================

/// `displayImportance` — важность отображения (EDT элемент / Designer атрибут).
pub const F_DISPLAY_IMPORTANCE: FieldId = FieldId(1);
/// `title` — локализованный заголовок поля (оба формата; Designer БЕЗ formatted-атрибута).
pub const F_TITLE: FieldId = FieldId(2);
/// `titleTextColor` — цвет текста заголовка (Color-кодек: EDT `core:ColorRef` / Designer `style:X`).
pub const F_TITLE_TEXT_COLOR: FieldId = FieldId(3);
/// `visible` — видимость (противоположные bool-дефолты: EDT эмитит true, Designer — false).
pub const F_VISIBLE: FieldId = FieldId(4);
/// `enabled` — доступность (EDT эмитит ВСЕГДА; Designer опускает true).
pub const F_ENABLED: FieldId = FieldId(5);
/// `userVisible` — пользовательская видимость (CommonBool: EDT `<common>true`/`<tag/>`;
/// Designer absent=true / `<xr:Common>false`).
pub const F_USER_VISIBLE: FieldId = FieldId(6);
/// `dataPath` — путь данных (DUAL-ENCODING, см. модульный док).
pub const F_DATA_PATH: FieldId = FieldId(7);
/// `defaultItem` — элемент по умолчанию (Bool, симметрично).
pub const F_DEFAULT_ITEM: FieldId = FieldId(8);
/// `skipOnInput` — пропускать при вводе (Bool, симметрично).
pub const F_SKIP_ON_INPUT: FieldId = FieldId(9);
/// `titleLocation` — положение заголовка (Enum, симметрично; оба опускают Auto).
pub const F_TITLE_LOCATION: FieldId = FieldId(10);
/// `titleHeight` — высота заголовка (Int, симметрично).
pub const F_TITLE_HEIGHT: FieldId = FieldId(11);
/// `toolTip` — локализованная подсказка (оба формата).
pub const F_TOOL_TIP: FieldId = FieldId(12);
/// `toolTipRepresentation` — представление подсказки (Enum, симметрично).
pub const F_TOOL_TIP_REPRESENTATION: FieldId = FieldId(13);
/// `readOnly` — только просмотр (Bool, симметрично).
pub const F_READ_ONLY: FieldId = FieldId(14);
/// `horizontalAlign` — горизонтальное выравнивание значения (Enum, симметрично).
pub const F_HORIZONTAL_ALIGN: FieldId = FieldId(15);
/// `groupHorizontalAlign` — выравнивание в группе по горизонтали (Enum, симметрично).
pub const F_GROUP_HORIZONTAL_ALIGN: FieldId = FieldId(16);
/// `verticalAlign` — вертикальное выравнивание (Enum, симметрично).
pub const F_VERTICAL_ALIGN: FieldId = FieldId(17);
/// `groupVerticalAlign` — выравнивание в группе по вертикали (Enum, симметрично).
pub const F_GROUP_VERTICAL_ALIGN: FieldId = FieldId(18);
/// `warningOnEditRepresentation` — представление предупреждения редактирования (Enum, симметрично).
pub const F_WARNING_ON_EDIT_REPRESENTATION: FieldId = FieldId(19);
/// `editMode` — режим редактирования (спец-кодировка Designer, см. модульный док).
pub const F_EDIT_MODE: FieldId = FieldId(20);
/// `fixingInTable` — фиксация в таблице (Enum, симметрично).
pub const F_FIXING_IN_TABLE: FieldId = FieldId(21);
/// `cellHyperlink` — ячейка-гиперссылка (Bool, симметрично).
pub const F_CELL_HYPERLINK: FieldId = FieldId(22);
/// `autoCellHeight` — авто-высота ячейки (Bool, симметрично).
pub const F_AUTO_CELL_HEIGHT: FieldId = FieldId(23);
/// `showInHeader` — показывать в шапке (противоположные bool-дефолты).
pub const F_SHOW_IN_HEADER: FieldId = FieldId(24);
/// `headerPicture` — картинка шапки (Picture-кодек: EDT `core:PictureRef` /
/// Designer `xr:Ref`+`xr:LoadTransparent`-денормализация).
pub const F_HEADER_PICTURE: FieldId = FieldId(25);
/// `headerHorizontalAlign` — выравнивание шапки (keep: EDT всегда, Designer опускает Left).
pub const F_HEADER_HORIZONTAL_ALIGN: FieldId = FieldId(26);
/// `showInFooter` — показывать в подвале (противоположные bool-дефолты).
pub const F_SHOW_IN_FOOTER: FieldId = FieldId(27);
/// `autoWidthInTable` — авто-ширина в таблице (Enum, симметрично).
pub const F_AUTO_WIDTH_IN_TABLE: FieldId = FieldId(28);
/// `cellHyperlinkRepresentation` — представление гиперссылки ячейки (Enum, симметрично).
pub const F_CELL_HYPERLINK_REPRESENTATION: FieldId = FieldId(29);
/// `cellHyperlinkDisplayVariant` — вариант отображения гиперссылки (Enum, симметрично).
pub const F_CELL_HYPERLINK_DISPLAY_VARIANT: FieldId = FieldId(30);
/// `footerHorizontalAlign` — выравнивание подвала (Enum, симметрично).
pub const F_FOOTER_HORIZONTAL_ALIGN: FieldId = FieldId(31);
/// `showTitleInCard` — показывать заголовок в карточке (Bool, симметрично; эмитится false).
pub const F_SHOW_TITLE_IN_CARD: FieldId = FieldId(32);
/// `shortcut` — горячая клавиша (Str, симметрично; метамодель FormField после `titleHeight`,
/// в HEAD-регионе — до `handlers`/`extendedTooltip`/`type`).
pub const F_SHORTCUT: FieldId = FieldId(33);
/// `warningOnEdit` — текст предупреждения при редактировании (Localized, симметрично;
/// метамодель после `warningOnEditRepresentation`).
pub const F_WARNING_ON_EDIT: FieldId = FieldId(34);
/// `markRequiredComplete` — помечать обязательность заполнения (Bool, симметрично; метамодель
/// после `warningOnEdit`, до `horizontalAlign`).
pub const F_MARK_REQUIRED_COMPLETE: FieldId = FieldId(35);

/// ECORE-дефолт `editMode` метамодели EDT (EDT ОПУСКАЕТ `Directly`). НЕ платформенный дефолт —
/// в каноне хранится ЯВНО.
pub const EDIT_MODE_EDT_DEFAULT: &str = "Directly";
/// ПЛАТФОРМЕННЫЙ дефолт `editMode` (Designer-дамп ОПУСКАЕТ `Enter`; EDT пишет его ЯВНО).
/// X-КАНОН: это значение в bag НЕ хранится — «свойство не задано» = отсутствие поля, одинаково
/// у обоих ридеров (кросс-витнесс ERP: EDT `Enter` ⟺ Designer отсутствует ×86 533).
pub const EDIT_MODE_DESIGNER_DEFAULT: &str = "Enter";
/// Литерал `editMode = Auto` (Designer кодирует парой EnterOnInput + AutoEditMode=true).
pub const EDIT_MODE_AUTO: &str = "Auto";
/// Designer-разрешение `Auto` (эмитится в `<EditMode>` при AutoEditMode=true).
pub const EDIT_MODE_AUTO_RESOLVED: &str = "EnterOnInput";
/// ПЛАТФОРМЕННЫЙ дефолт `headerHorizontalAlign` (Designer-дамп опускает `Left`; EDT пишет его
/// ЯВНО). X-КАНОН: в bag НЕ хранится — «не задано» = отсутствие поля у ОБОИХ ридеров
/// (кросс-витнесс ERP: EDT `Left` ⟺ Designer отсутствует ×122 142).
pub const HEADER_HORIZONTAL_ALIGN_DEFAULT: &str = "Left";
/// ECORE-дефолт `headerHorizontalAlign` метамодели EDT (EDT опускает `Auto`; сверено корпусом
/// покрытия s10_forms — поле с `Auto` опускается в EDT, а `Left`/`Center`/`Right` эмитятся).
/// НЕ платформенный дефолт (тот — `Left`), поэтому в каноне хранится ЯВНО.
pub const HEADER_HORIZONTAL_ALIGN_EDT_DEFAULT: &str = "Auto";

// ============================ extInfo: InputField ============================

/// extInfo: `width` (Int, симметрично).
pub const F_EXT_WIDTH: FieldId = FieldId(101);
/// extInfo: `autoMaxWidth` (противоположные bool-дефолты).
pub const F_EXT_AUTO_MAX_WIDTH: FieldId = FieldId(102);
/// extInfo: `height` (Int, симметрично).
pub const F_EXT_HEIGHT: FieldId = FieldId(103);
/// extInfo: `maxWidth` (Int, симметрично).
pub const F_EXT_MAX_WIDTH: FieldId = FieldId(104);
/// extInfo: `autoMaxHeight` (противоположные bool-дефолты).
pub const F_EXT_AUTO_MAX_HEIGHT: FieldId = FieldId(105);
/// extInfo: `horizontalStretch` (Bool, симметрично — ОБА эмитят явные true/false).
pub const F_EXT_HORIZONTAL_STRETCH: FieldId = FieldId(106);
/// extInfo: `verticalStretch` (Bool, симметрично).
pub const F_EXT_VERTICAL_STRETCH: FieldId = FieldId(107);
/// extInfo: `wrap` (противоположные bool-дефолты).
pub const F_EXT_WRAP: FieldId = FieldId(108);
/// extInfo: `choiceListButton` (Bool, симметрично).
pub const F_EXT_CHOICE_LIST_BUTTON: FieldId = FieldId(109);
/// extInfo: `dropListButton` (Bool, симметрично).
pub const F_EXT_DROP_LIST_BUTTON: FieldId = FieldId(110);
/// extInfo: `extendedEditMultipleValues` (Bool, симметрично).
pub const F_EXT_EXTENDED_EDIT_MULTIPLE_VALUES: FieldId = FieldId(111);
/// extInfo: `multiLine` (Bool, симметрично).
pub const F_EXT_MULTI_LINE: FieldId = FieldId(112);
/// extInfo: `passwordMode` (Bool, симметрично).
pub const F_EXT_PASSWORD_MODE: FieldId = FieldId(113);
/// extInfo: `choiceButton` (Bool, симметрично).
pub const F_EXT_CHOICE_BUTTON: FieldId = FieldId(114);
/// extInfo: `choiceButtonPicture` (Picture-кодек).
pub const F_EXT_CHOICE_BUTTON_PICTURE: FieldId = FieldId(115);
/// extInfo: `choiceButtonRepresentation` (Enum, симметрично).
pub const F_EXT_CHOICE_BUTTON_REPRESENTATION: FieldId = FieldId(116);
/// extInfo: `clearButton` (Bool, симметрично).
pub const F_EXT_CLEAR_BUTTON: FieldId = FieldId(117);
/// extInfo: `format` (Localized, симметрично).
pub const F_EXT_FORMAT: FieldId = FieldId(118);
/// extInfo: `listChoiceMode` (Bool, симметрично).
pub const F_EXT_LIST_CHOICE_MODE: FieldId = FieldId(119);
/// extInfo: `openButton` (Bool, симметрично).
pub const F_EXT_OPEN_BUTTON: FieldId = FieldId(120);
/// extInfo: `autoMarkIncomplete` (Bool, симметрично).
pub const F_EXT_AUTO_MARK_INCOMPLETE: FieldId = FieldId(121);
/// extInfo: `quickChoice` (Bool, симметрично).
pub const F_EXT_QUICK_CHOICE: FieldId = FieldId(122);
/// extInfo: `spinButton` (Bool, симметрично).
pub const F_EXT_SPIN_BUTTON: FieldId = FieldId(123);
/// extInfo: `editFormat` (Localized, симметрично).
pub const F_EXT_EDIT_FORMAT: FieldId = FieldId(124);
/// extInfo: `chooseType` (противоположные bool-дефолты: EDT true / Designer false).
pub const F_EXT_CHOOSE_TYPE: FieldId = FieldId(125);
/// extInfo: `incompleteChoiceMode` (Enum, симметрично).
pub const F_EXT_INCOMPLETE_CHOICE_MODE: FieldId = FieldId(126);
/// extInfo: `typeDomainEnabled` (keep: EDT всегда; Designer НИКОГДА — fill `true`).
pub const F_EXT_TYPE_DOMAIN_ENABLED: FieldId = FieldId(127);
/// extInfo: `textEdit` (противоположные bool-дефолты).
pub const F_EXT_TEXT_EDIT: FieldId = FieldId(128);
/// extInfo: `backColor` (Color-кодек).
pub const F_EXT_BACK_COLOR: FieldId = FieldId(129);
/// extInfo: `editTextUpdate` (Enum, симметрично).
pub const F_EXT_EDIT_TEXT_UPDATE: FieldId = FieldId(130);
/// extInfo: `minValue`. InputField — nullable-скаляр xsi-вида (общий value_codec:
/// `core:NumberValue`-decimal/`core:StringValue`/Designer `xs:decimal`/`xs:string`); TrackBar
/// РЕИСПОЛЬЗУЕТ этот id как plain Int (per-control value_kind).
pub const F_EXT_MIN_VALUE: FieldId = FieldId(131);
/// extInfo: `maxValue` (InputField — Value-скаляр; TrackBar/ProgressBar — plain Int).
pub const F_EXT_MAX_VALUE: FieldId = FieldId(132);
/// extInfo: `textColor` (Color-кодек).
pub const F_EXT_TEXT_COLOR: FieldId = FieldId(133);
/// extInfo: `textSize` (keep: EDT всегда `Normal`; Designer НИКОГДА — fill `Normal`).
pub const F_EXT_TEXT_SIZE: FieldId = FieldId(134);
/// extInfo: `heightControlVariant` (Enum-МЭППИНГ: EDT `ByContent` ⟺ Designer `UseContentHeight`).
pub const F_EXT_HEIGHT_CONTROL_VARIANT: FieldId = FieldId(135);
/// extInfo: `inputHint` (Localized, симметрично).
pub const F_EXT_INPUT_HINT: FieldId = FieldId(136);
/// extInfo: `multipleValueDataPath` (DataPath dual-encoding, как `dataPath`).
pub const F_EXT_MULTIPLE_VALUE_DATA_PATH: FieldId = FieldId(137);
/// extInfo: `choiceList` — список значений выбора (repeatable, structured ValueList).
/// DUAL-ENCODING: EDT повторяемый `<choiceList><presentation/><value xsi:type="core:…Value">`
/// ⟺ Designer один `<ChoiceList><xr:Item>…<xr:Value xsi:type="FormChoiceListDesTimeValue">`.
/// Канон — `List` из пар `[presentation:Localized, value:Value]` (X by construction).
/// EDT-позиция — между `textEdit` и `backColor` (сверено).
pub const F_EXT_CHOICE_LIST: FieldId = FieldId(138);
/// extInfo: `availableTypes` — описание доступных типов ввода (тип-значение [`TypeSpec`], через
/// общий type-codec). EDT `<availableTypes><types>String</types><stringQualifiers/></availableTypes>`
/// ⟺ Designer `<AvailableTypes><v8:Type>xs:string</v8:Type><v8:StringQualifiers>…`. Симметрично;
/// X by construction (тот же canon TypeSpec, что `<valueType>`/`<Type>`). Позиция — после `textEdit`
/// (сверено ФормаПоиска).
pub const F_EXT_AVAILABLE_TYPES: FieldId = FieldId(139);

/// extInfo: `allowInputEmptyMultipleValues` — разрешать ввод пустых множественных значений
/// (Bool, симметрично: оба формата эмитят `true`, оба опускают дефолт false — сверено
/// корпусом покрытия). EDT-позиция — сразу после `wrap`.
pub const F_EXT_ALLOW_INPUT_EMPTY_MULTIPLE_VALUES: FieldId = FieldId(140);
/// extInfo: `choiceParameters` — параметры выбора поля-ввода (repeatable, `name`+значение
/// дизайн-тайм). ОТЛИЧАЕТСЯ от дескрипторного `choiceParameters` тем, что значение обёрнуто в
/// `FormChoiceListDesTimeValue` (пустой `<Presentation/>` + скаляр `<Value>`). DUAL-ENCODING:
/// EDT повторяемый `<choiceParameters><name>Path</name><value xsi:type="form:FormChoiceListDesTimeValue">
/// <value xsi:type="core:…Value">…` ⟺ Designer контейнер `<ChoiceParameters><app:item name="Path">
/// <app:value xsi:type="FormChoiceListDesTimeValue"><Presentation/><Value xsi:type="xs:…">`. Канон —
/// `List` из пар `[name:Str, value:Value]` (X by construction; пустой Presentation реконструируется
/// пер-диалектно). EDT-позиция — перед `availableTypes` (метамодель: choiceParameters 49 → availableTypes 50).
pub const F_EXT_CHOICE_PARAMETERS: FieldId = FieldId(167);
/// extInfo InputField: `extendedEdit` — расширенное редактирование (Bool, симметрично: ОБА
/// формата эмитят фактическое значение, включая явный `false` — SSL 33⟷33, 28×true+5×false).
/// Метамодель `InputFieldExtInfo`: `multiLine` (12) → **extendedEdit (13)** →
/// `allowInputEmptyMultipleValues` (14); SSL-витнессы (Задание.ДействиеПроверить и др.):
/// EDT `wrap → multiLine → extendedEdit → chooseType`, Designer `MultiLine → ExtendedEdit`.
pub const F_EXT_EXTENDED_EDIT: FieldId = FieldId(168);
/// extInfo InputField: `borderColor` — цвет рамки (Color-кодек: EDT `core:ColorRef` /
/// Designer `style:X`/`pal:Y`; симметрично — SSL 42⟷42, значения зеркальны). Метамодель:
/// `backColor` (53) → **borderColor (54)** → `textSize` (55); SSL-витнессы: EDT
/// `textEdit → borderColor → textSize`, Designer `… → BorderColor → ContextMenu`.
pub const F_EXT_BORDER_COLOR: FieldId = FieldId(169);
/// extInfo InputField: `choiceForm` — форма выбора (Ref-текст `Catalog.X.Form.Y`, симметрично;
/// метамодель #47: maxValue → **choiceForm** → choiceParameterLinks; witness
/// МашиночитаемыеДоверенности.ФормаСписка: EDT textEdit→choiceForm→textSize ⟺ Designer
/// OpenButton→ChoiceForm→ContextMenu).
pub const F_EXT_CHOICE_FORM: FieldId = FieldId(246);
/// extInfo InputField: `choiceParameterLinks` — связи параметров выбора (repeatable,
/// structured; метамодель #48). Канон — `List` пунктов `[name:Str, path:Ref]`; Designer-каркас
/// `ValueChange=Clear` не хранится (см. кодек `ChoiceParameterLinks` форм-таблиц).
pub const F_EXT_CHOICE_PARAMETER_LINKS: FieldId = FieldId(247);
/// extInfo InputField: `typeLink` — связь по типу (structured; метамодель #63, перед
/// heightControlVariant). Канон — `List([path:Ref, linkItem:Int])` (EDT омитит linkItem=0;
/// Designer эмитит оба листа всегда).
pub const F_EXT_TYPE_LINK: FieldId = FieldId(248);

/// extInfo InputField: `maxHeight` — макс. высота (Int, симметрично). Метамодель
/// `InputFieldExtInfo` #7 (после `autoMaxHeight`, до `horizontalStretch`). Реиспользует id
/// [`F_EXT_MAX_HEIGHT`] (210) — та же каноника «maxHeight».
/// extInfo InputField: `picture` — картинка поля (PictureRef; метамодель #25, до
/// `choiceButtonPicture`).
pub const F_EXT_PICTURE: FieldId = FieldId(170);
/// extInfo InputField: `createButton` — кнопка создания (Bool, симметрично; метамодель #30,
/// после `openButton`).
pub const F_EXT_CREATE_BUTTON: FieldId = FieldId(171);
/// extInfo InputField: `mask` — маска ввода (Str, симметрично; метамодель #31, после
/// `createButton`).
pub const F_EXT_MASK: FieldId = FieldId(172);
/// extInfo InputField: `autoChoiceIncomplete` — авто-незавершённый выбор (Bool, симметрично;
/// метамодель #32, после `mask`).
pub const F_EXT_AUTO_CHOICE_INCOMPLETE: FieldId = FieldId(173);
/// extInfo InputField: `choiceListHeight` — высота списка выбора (Int, симметрично;
/// метамодель #38, после `listChoiceMode`).
pub const F_EXT_CHOICE_LIST_HEIGHT: FieldId = FieldId(174);
/// extInfo InputField: `autoShowOpenButtonMode` — режим авто-показа кнопки открытия (Enum,
/// симметрично; метамодель #66, после `heightControlVariant`).
pub const F_EXT_AUTO_SHOW_OPEN_BUTTON_MODE: FieldId = FieldId(175);
/// extInfo InputField: `autoCorrectionOnTextInput` — автокоррекция при вводе (Enum,
/// симметрично; метамодель #67, после `autoShowOpenButtonMode`).
pub const F_EXT_AUTO_CORRECTION_ON_TEXT_INPUT: FieldId = FieldId(176);
/// extInfo InputField: `dropListWidth` — ширина выпадающего списка (Int, симметрично;
/// метамодель #83, после `multipleValueDataPath`, до `choiceHistoryOnInput`).
pub const F_EXT_DROP_LIST_WIDTH: FieldId = FieldId(182);
/// extInfo PictureField (ImageField): `nonselectedPictureText` — текст при отсутствии картинки
/// (Localized; метамодель `ImageFieldExtInfo` после `hyperlink`, до `enableStartDrag`).
pub const F_EXT_NONSELECTED_PICTURE_TEXT: FieldId = FieldId(185);
/// extInfo InputField: `autoShowClearButtonMode` — режим авто-показа кнопки очистки (Enum,
/// симметрично; witness `FilledOnly`). Метамодель — рядом с `autoShowOpenButtonMode`.
pub const F_EXT_AUTO_SHOW_CLEAR_BUTTON_MODE: FieldId = FieldId(186);
/// extInfo InputField: `spellCheckingOnTextInput` — проверка орфографии при вводе (Enum,
/// симметрично; witness `DontUse`). Метамодель — после `autoShowOpenButtonMode`/
/// `autoCorrectionOnTextInput`, до `inputHint`.
pub const F_EXT_SPELL_CHECKING_ON_TEXT_INPUT: FieldId = FieldId(187);

/// Designer-fill `typeDomainEnabled` (EDT-эмитимая константа корпуса, 288/288 true).
pub const TYPE_DOMAIN_ENABLED_FILL: &str = "true";
/// Designer-fill `textSize` — Designer ОПУСКАЕТ свой дефолт `Normal` (2738/2820 InputField SSL).
pub const TEXT_SIZE_FILL: &str = "Normal";
/// EDT-fill `textSize` — EDT-ДЕФОЛТ `Enlarged` (EMF-конвенция: дефолт EEnum-атрибута = ПЕРВЫЙ
/// литерал перечисления), поэтому EDT эмитит `<textSize>Normal</textSize>` явно (2738/2820) и
/// ОПУСКАЕТ тег ровно на тех 82 полях, где значение `Enlarged`.
///
/// Ранняя волна приняла эту омиссию за НЕРЕГУЛЯРНОСТЬ («§1.0-неоднозначность класса textSize
/// 82/2820») и хранила EDT-сторону presence-точно (`Policy::DesKeep`) — из-за чего IR двух
/// диалектов расходился и designer→cf ОТКАЗЫВАЛ на литерале `Enlarged`. Она регулярна:
/// контингентная таблица (EDT × Designer) по всем 2820 InputField'ам SSL ЧИСТАЯ, ровно две клетки
/// и НИ ОДНОЙ внедиагональной — `EDT <ABSENT>` ⟺ `Designer Enlarged` (82), `Normal` ⟺ `Normal`
/// (2738). См. `probe_defaults`.
pub const TEXT_SIZE_EDT_FILL: &str = "Enlarged";

// ============================ extInfo: CheckBoxField ============================

/// extInfo: `checkBoxType` (Enum, симметрично: CheckBox/Switcher).
pub const F_EXT_CHECK_BOX_TYPE: FieldId = FieldId(141);
/// extInfo: `threeState` (Bool, симметрично).
pub const F_EXT_THREE_STATE: FieldId = FieldId(142);

// ============================ extInfo: LabelField ============================

/// extInfo: `hyperlink` — Bool; ВНИМАНИЕ: Designer-тег `Hiperlink` (опечатка платформы).
pub const F_EXT_HYPERLINK: FieldId = FieldId(151);
/// extInfo: `useCopy` (Bool, симметрично).
pub const F_EXT_USE_COPY: FieldId = FieldId(152);

// ============ extInfo: document-поля (HTML/ProgressBar/Formatted) ============
// Общая геометрия (width/autoMax*/height/horizontalStretch/verticalStretch) РЕИСПОЛЬЗУЕТ
// id 101-107: у этих типов Designer её ОПУСКАЕТ ВСЕГДА (все дефолтные), поэтому позиция в
// пуловом Designer-порядке нерелевантна, а EDT эмитит по своей ПЕР-ТИПОВОЙ таблице.

/// extInfo (HtmlField): `output` — режим вывода (Enum, симметрично; дефолт `Use` опускается).
pub const F_EXT_OUTPUT: FieldId = FieldId(160);
/// extInfo (ProgressBarField): `showPercent` (Bool; Designer эмитит).
pub const F_EXT_SHOW_PERCENT: FieldId = FieldId(161);
/// extInfo (ProgressBar/TrackBar): `orientation` — ориентация (Enum, симметрично: оба
/// формата эмитят `Vertical`, оба опускают дефолт `Horizontal`; сверено корпусом покрытия).
pub const F_EXT_ORIENTATION: FieldId = FieldId(162);
/// extInfo InputField: `allowMultipleValuesDuplicates` — разрешать дубли множественных
/// значений (Bool, симметрично: оба эмитят `true`, оба опускают дефолт false).
pub const F_EXT_ALLOW_MULTIPLE_VALUES_DUPLICATES: FieldId = FieldId(163);
/// extInfo InputField: `choiceFoldersAndItems` — выбор групп и элементов (Enum, симметрично:
/// оба эмитят `Items`/`Folders`/`FoldersAndItems`, оба опускают дефолт `Auto`).
pub const F_EXT_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(164);
/// extInfo ProgressBarField: `representation` — представление индикатора (Enum, симметрично:
/// оба эмитят `Broken`/`BrokenTilt`/…, оба опускают дефолт `Auto`).
pub const F_EXT_PROGRESS_REPRESENTATION: FieldId = FieldId(165);
/// extInfo InputField: `choiceHistoryOnInput` — история выбора при вводе (Enum, симметрично:
/// оба эмитят `DontUse`, оба опускают дефолт `Auto`). EDT-позиция — после `textSize`.
pub const F_EXT_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(166);

// ============ extInfo: TrackBarField (form:TrackBarFieldExtInfo) ============
// Геометрия РЕИСПОЛЬЗУЕТ id 101-107 (width/autoMaxWidth/height/autoMaxHeight/horizontal-
// verticalStretch); `maxValue` РЕИСПОЛЬЗУЕТ 132; `orientation` — 162. Тип-специфичные поля —
// блок 240+. Designer ОПУСКАЕТ всю геометрию/шаги (тип-дефолты); эмитит лишь не-дефолт.

/// extInfo (TrackBarField): `step` — шаг (Int, KEEP: EDT всегда / Designer опускает `1`).
pub const F_EXT_STEP: FieldId = FieldId(240);
/// extInfo (TrackBarField): `largeStep` — большой шаг (Int, KEEP: EDT всегда / Designer опускает `10`).
pub const F_EXT_LARGE_STEP: FieldId = FieldId(241);
/// extInfo (TrackBarField): `markingStep` — шаг отметок (Int, KEEP: EDT всегда / Designer опускает `5`).
pub const F_EXT_MARKING_STEP: FieldId = FieldId(242);
/// extInfo (TrackBarField): `markingAppearance` — расположение отметок (Enum, KEEP: EDT
/// всегда / Designer опускает `BottomRight`).
pub const F_EXT_MARKING_APPEARANCE: FieldId = FieldId(243);

/// TrackBarField-дефолт `width` (Designer опускает `32`).
pub const TRACK_BAR_WIDTH_DEFAULT: &str = "32";
/// TrackBarField-дефолт `height` (Designer опускает `2`).
pub const TRACK_BAR_HEIGHT_DEFAULT: &str = "2";
/// TrackBarField-дефолт `maxValue` (Designer опускает `100`).
pub const TRACK_BAR_MAX_VALUE_DEFAULT: &str = "100";
/// TrackBarField-дефолт `step` (Designer опускает `1`).
pub const TRACK_BAR_STEP_DEFAULT: &str = "1";
/// TrackBarField-дефолт `largeStep` (Designer опускает `10`).
pub const TRACK_BAR_LARGE_STEP_DEFAULT: &str = "10";
/// TrackBarField-дефолт `markingStep` (Designer опускает `5`).
pub const TRACK_BAR_MARKING_STEP_DEFAULT: &str = "5";
/// Designer-дефолт `markingAppearance` (Designer опускает `BottomRight`).
pub const TRACK_BAR_MARKING_APPEARANCE_DEFAULT: &str = "BottomRight";
/// EDT-дефолт `markingAppearance` (EDT опускает `DontShow`; сверено корпусом покрытия —
/// вариант `MarkingAppearance_НеПоказывать` несёт ПУСТОЙ markingAppearance в EDT, а
/// `BottomRight`/`TopLeft`/`BothSides` эмитятся). ПРОТИВОПОЛОЖНЫЕ дефолты (DontShow ⟺ BottomRight).
pub const TRACK_BAR_MARKING_APPEARANCE_EDT_DEFAULT: &str = "DontShow";

// ============================ extInfo: ImageField (PictureField) ============================
// Геометрия РЕИСПОЛЬЗУЕТ id 101-107 (width/autoMaxWidth/maxWidth/height/autoMaxHeight/
// horizontalStretch/verticalStretch); тип-специфичные поля — собственный смежный блок 177+
// (Designer эмитит их ПОСЛЕ геометрии, в EDT-порядке).

/// extInfo (ImageField): `pictureSize` (Enum, симметрично).
pub const F_EXT_PICTURE_SIZE: FieldId = FieldId(177);
/// extInfo (ImageField): `hyperlink` — Bool (Designer-тег `Hyperlink`, БЕЗ опечатки — в
/// отличие от LabelField `Hiperlink`; поэтому СОБСТВЕННЫЙ id, не 151).
pub const F_EXT_PIC_HYPERLINK: FieldId = FieldId(178);
/// extInfo (ImageField): `pictureColor` (Color-кодек; метамодель ПЕРЕД `valuesPicture`).
/// Witness — DocumentJournal Взаимодействия ImageFieldExtInfo pictureColor=Palette.Red.
pub const F_EXT_PICTURE_COLOR: FieldId = FieldId(244);
/// extInfo (ImageField): `textColor` (Color-кодек; метамодель ПОСЛЕ `valuesPicture`, ДО `border`).
/// СОБСТВЕННЫЙ id (не 133): Designer-порядок ImageField textColor — ПОСЛЕ ValuesPicture, тогда
/// как InputField `textColor` (133) — раньше; общий DES-слот не выразил бы обе позиции.
pub const F_EXT_PIC_TEXT_COLOR: FieldId = FieldId(245);
/// extInfo (ImageField): `valuesPicture` (PictureRef-кодек: EDT `core:PictureRef` /
/// Designer `xr:Ref`+`xr:LoadTransparent`).
pub const F_EXT_VALUES_PICTURE: FieldId = FieldId(179);
/// extInfo (ImageField): `border` (KEEP-композит `Border`, см. `fields.rs::Codec::Border`):
/// EDT `<border xsi:type="core:BorderDef">[<style>Single</style>]<width>1</width></border>`
/// (ВСЕГДА эмитит) ⟺ Designer `<Border width="1"><v8ui:style>WithoutBorder</v8ui:style>`
/// (эмитит ТОЛЬКО не-Single). Канон — `Enum(Single|WithoutBorder)`; ширина всегда 1.
pub const F_EXT_BORDER: FieldId = FieldId(180);
/// extInfo (ImageField): `fileDragMode` (KEEP: EDT опускает `AsFile`, Designer опускает
/// `AsFileRef` — те же дефолты, что PictureDecoration).
pub const F_EXT_FILE_DRAG_MODE: FieldId = FieldId(181);

/// Канон-литерал border «одинарная рамка» (EDT эмитит `<style>Single`, Designer опускает).
pub const BORDER_STYLE_SINGLE: &str = "Single";
/// Канон-литерал border «без рамки» (EDT опускает `<style>`, Designer эмитит `WithoutBorder`).
pub const BORDER_STYLE_WITHOUT: &str = "WithoutBorder";
/// EDT-дефолт `fileDragMode` (омиссия `AsFile`; Designer эмитит `AsFile` явно).
pub const FILE_DRAG_MODE_EDT_DEFAULT: &str = "AsFile";
/// Designer-дефолт `fileDragMode` (омиссия `AsFileRef`; EDT эмитит `AsFileRef` явно).
pub const FILE_DRAG_MODE_DESIGNER_DEFAULT: &str = "AsFileRef";

// ============ extInfo: SpreadsheetDocumentField (LANE-F-6) ============
// `form:SpreadSheetDocFieldExtInfo` (обратите внимание на регистр — SpreadSheet, как у
// Designer-тега `<SpreadSheetDocumentField>`, тогда как EDT `<type>` = `SpreadsheetDocumentField`).
// Геометрия РЕИСПОЛЬЗУЕТ id 101-107 (width/autoMaxWidth/height/autoMaxHeight/horizontal-
// verticalStretch); `output` РЕИСПОЛЬЗУЕТ 160. Тип-специфичные поля — блок 210+.
//
// Пер-форматные дефолты (cross-omission witness по 7 CommonForms, 12 экземпляров):
// * `width`/`height` — EDT эмитит ВСЕГДА; Designer опускает свой дефолт (50 / 10) — KEEP.
// * `pointerType`(=Special)/`drawingSelectionShowMode`(=Auto)/`showGroups`(=true) — EDT
//   эмитит ВСЕГДА, Designer НЕ несёт никогда (DesOmit::Always).
// * `selectionShowMode` — ПРОТИВОПОЛОЖНЫЕ дефолты: EDT опускает `WhenActive`, Designer
//   опускает `Always` (KEEP).
// * `verticalScrollBar`/`horizontalScrollBar` — ENUM↔BOOL кросс-кодировка: EDT enum
//   {ScrollAuto, ScrollAlways, ScrollNever}; Designer BOOL (`true`=ScrollAlways,
//   `false`=ScrollNever, ОПУСК=ScrollAuto). EDT опускает `ScrollNever` (свой дефолт),
//   Designer опускает `ScrollAuto` (KEEP + Codec::ScrollBar).
// * `enableStartDrag`/`enableDrag`/geometry autoMax*/stretch — OppositeBool.
// * `maxHeight`/`showGrid`/`showHeaders`/`showCellNames`/`showRowAndColumnNames`/`output`/
//   `edit` — Symmetric (оба эмитят фактическое значение).

/// extInfo (SpreadsheetDocumentField): `maxHeight` (Int, симметрично).
pub const F_EXT_MAX_HEIGHT: FieldId = FieldId(210);
/// extInfo: `showGrid` (Bool, симметрично).
pub const F_EXT_SHOW_GRID: FieldId = FieldId(211);
/// extInfo: `showHeaders` (Bool, симметрично).
pub const F_EXT_SHOW_HEADERS: FieldId = FieldId(212);
/// extInfo: `showCellNames` (Bool, симметрично).
pub const F_EXT_SHOW_CELL_NAMES: FieldId = FieldId(213);
/// extInfo: `showRowAndColumnNames` (Bool, симметрично).
pub const F_EXT_SHOW_ROW_AND_COLUMN_NAMES: FieldId = FieldId(214);
/// extInfo: `pointerType` (keep: EDT всегда `Special`, Designer НИКОГДА — fill `Special`).
pub const F_EXT_POINTER_TYPE: FieldId = FieldId(215);
/// extInfo: `verticalScrollBar` (ScrollBar-кодек: EDT enum ⟺ Designer bool; KEEP).
pub const F_EXT_VERTICAL_SCROLL_BAR: FieldId = FieldId(216);
/// extInfo: `horizontalScrollBar` (ScrollBar-кодек, как выше).
pub const F_EXT_HORIZONTAL_SCROLL_BAR: FieldId = FieldId(217);
/// extInfo: `selectionShowMode` (keep: EDT опускает `WhenActive`, Designer опускает `Always`).
pub const F_EXT_SELECTION_SHOW_MODE: FieldId = FieldId(218);
/// extInfo: `drawingSelectionShowMode` (keep: EDT всегда `Auto`, Designer НИКОГДА).
pub const F_EXT_DRAWING_SELECTION_SHOW_MODE: FieldId = FieldId(219);
/// extInfo: `edit` (Bool, симметрично).
pub const F_EXT_EDIT: FieldId = FieldId(220);
/// extInfo: `showGroups` (keep Bool: EDT всегда `true`, Designer НИКОГДА).
pub const F_EXT_SHOW_GROUPS: FieldId = FieldId(221);
/// extInfo: `enableStartDrag` (OppositeBool: EDT эмитит true / Designer эмитит false).
pub const F_EXT_ENABLE_START_DRAG: FieldId = FieldId(222);
/// extInfo: `enableDrag` (OppositeBool).
pub const F_EXT_ENABLE_DRAG: FieldId = FieldId(223);
/// extInfo: `protection` — защита табличного документа (Bool, Symmetric: оба эмитят `true`,
/// оба опускают дефолт false). Метамодель SpreadSheetDocFieldExtInfo: horizontalScrollBar →
/// blackAndWhiteView → **protection** → selectionShowMode. Witness — СнимкиОтчетов ×2 форм:
/// EDT horizontalScrollBar→protection→selectionShowMode; Designer …→Protection→ContextMenu.
pub const F_EXT_PROTECTION: FieldId = FieldId(224);
/// extInfo (LabelField): `border` — рамка надписи-поля ([`crate::spec::forms::controls`]
/// Border-кодек; Symmetric: оба эмитят не-дефолт, оба опускают отсутствие). СОБСТВЕННЫЙ id
/// (НЕ [`F_EXT_BORDER`]): Designer-слот LabelField (VerticalStretch→Border→BackColor, witness
/// ПоискИУдалениеДублей.ПоискДублей Single⟷Single) НЕСОВМЕСТИМ со слотом PictureField
/// (ValuesPicture→TextColor→Border→FileDragMode) — пуловый DES-порядок требует раздельных id.
/// Метамодель LabelFieldExtInfo: passwordMode(13) → **border(14)** → borderColor → textColor.
pub const F_EXT_LABEL_BORDER: FieldId = FieldId(225);
/// ОБЩЕЕ тело: `footerText` — текст подвала колонки (Localized, Symmetric). Метамодель
/// FormField: showInFooter(46) → footerDataPath → **footerText(48)** → … → footerHorizontalAlign.
/// Witness — СообщениеSMS.ФормаДокумента ×2 InputField-колонки: EDT showInFooter→footerText→
/// extInfo; Designer EditMode→FooterText→AutoEditMode.
pub const F_FOOTER_TEXT: FieldId = FieldId(226);

/// Designer-fill/EDT-fill `pointerType` (EDT-эмитимая константа корпуса, 12/12 `Special`).
pub const POINTER_TYPE_FILL: &str = "Special";
/// Designer-fill/EDT-fill `drawingSelectionShowMode` (12/12 `Auto`).
pub const DRAWING_SELECTION_SHOW_MODE_FILL: &str = "Auto";
/// EDT-дефолт `selectionShowMode` (EDT опускает `WhenActive`).
pub const SELECTION_SHOW_MODE_EDT_DEFAULT: &str = "WhenActive";
/// Designer-дефолт `selectionShowMode` (Designer опускает `Always`).
pub const SELECTION_SHOW_MODE_DESIGNER_DEFAULT: &str = "Always";
/// SpreadsheetDocumentField-дефолт `width` (Designer опускает `50`; EDT эмитит всегда).
pub const SS_WIDTH_DEFAULT: &str = "50";
/// SpreadsheetDocumentField-дефолт `height` (Designer опускает `10`; EDT эмитит всегда).
pub const SS_HEIGHT_DEFAULT: &str = "10";
/// EDT-дефолт scroll-бара (EDT опускает `ScrollNever`; Designer кодирует его как `false`).
pub const SCROLL_BAR_EDT_DEFAULT: &str = "ScrollNever";
/// Designer-дефолт scroll-бара (Designer опускает `ScrollAuto`; EDT эмитит его явно).
pub const SCROLL_BAR_DESIGNER_DEFAULT: &str = "ScrollAuto";
/// Канон scroll-бара «всегда» (Designer кодирует `true`).
pub const SCROLL_BAR_ALWAYS: &str = "ScrollAlways";
/// Канон scroll-бара «никогда» (Designer кодирует `false`).
pub const SCROLL_BAR_NEVER: &str = "ScrollNever";

// ============ extInfo: CalendarField (LANE-F-7) ============
// `form:CalendarFieldExtInfo`. Геометрия РЕИСПОЛЬЗУЕТ id 101-107 (width/autoMaxWidth/height/
// autoMaxHeight/horizontalStretch/verticalStretch); `border` РЕИСПОЛЬЗУЕТ 180. Тип-специфичные
// поля — блок 230+. Cross-omission witness (форма `ВыборДаты`, 1 экземпляр в CommonForms):
// * `width`/`height` — Symmetric (ОБА эмитят фактическое значение).
// * `autoMax*`/`horizontalStretch`/`verticalStretch`/`calendarNavigation` — OppositeBool
//   (EDT эмитит true / Designer опускает свой дефолт true).
// * `showCurrentDate` — OppositeBool (EDT опускает false / Designer эмитит false).
// * `border` — KEEP-композит `Border` (EDT всегда `Single` / Designer опускает `Single`).
// * `widthInMonths` — Symmetric (Int; оба эмитят фактическое значение).
// * `heightInMonths` — KEEP Int (EDT эмитит ВСЕГДА / Designer опускает дефолт `1`).

/// extInfo (CalendarField): `calendarNavigation` (OppositeBool: EDT true / Designer default true).
pub const F_EXT_CALENDAR_NAVIGATION: FieldId = FieldId(230);
/// extInfo (CalendarField): `showCurrentDate` (OppositeBool: EDT default false / Designer эмитит false).
pub const F_EXT_SHOW_CURRENT_DATE: FieldId = FieldId(231);
/// extInfo (CalendarField): `widthInMonths` (Int, Symmetric).
pub const F_EXT_WIDTH_IN_MONTHS: FieldId = FieldId(232);
/// extInfo (CalendarField): `heightInMonths` (KEEP Int: EDT всегда / Designer опускает `1`).
pub const F_EXT_HEIGHT_IN_MONTHS: FieldId = FieldId(233);
/// extInfo (CalendarField): `selectionMode` — режим выделения (Enum, симметрично:
/// `Multiple`/`Interval`/…; оба опускают дефолт `Single`). EDT — после `verticalStretch`.
pub const F_EXT_CALENDAR_SELECTION_MODE: FieldId = FieldId(234);
/// extInfo (CalendarField): `showMonthsPanel` (Bool, симметрично: оба эмитят `true`, оба
/// опускают дефолт false). EDT — после `border`.
pub const F_EXT_SHOW_MONTHS_PANEL: FieldId = FieldId(235);

/// extInfo (PDFDocumentField): `scale` — масштаб просмотра (Int; EDT эмитит ВСЕГДА `100` /
/// Designer опускает дефолт `100`). Witness — DataProcessor.СервисДоставки.ПросмотрPDF.
pub const F_EXT_SCALE: FieldId = FieldId(249);
/// extInfo (PDFDocumentField): `currentPageNumber` — текущая страница (Int; EDT эмитит ВСЕГДА
/// `1` / Designer опускает дефолт `1`). Witness — DataProcessor.СервисДоставки.ПросмотрPDF.
pub const F_EXT_CURRENT_PAGE_NUMBER: FieldId = FieldId(250);

/// Дефолт `heightInMonths` (Designer опускает `1`; EDT эмитит всегда).
pub const HEIGHT_IN_MONTHS_DEFAULT: &str = "1";

/// Дефолт `scale` PDFDocumentField (Designer опускает `100`; EDT эмитит всегда).
pub const PDF_SCALE_DEFAULT: &str = "100";
/// Дефолт `currentPageNumber` PDFDocumentField (Designer опускает `1`; EDT эмитит всегда).
pub const PDF_CURRENT_PAGE_NUMBER_DEFAULT: &str = "1";
