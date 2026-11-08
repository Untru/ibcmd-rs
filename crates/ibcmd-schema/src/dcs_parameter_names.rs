//! Standard DCS parameter keys in Russian- and English-script configurations.
//!
//! Native 8.3.27.2214 / 8.5.1.1529 counterexamples differ only in
//! ScriptVariant; the public English composition, structured and nested
//! context sets exercise these keys. User data-parameter names are separate.

/// The English spelling of an authenticated standard Russian parameter key.
/// Unknown names are not inferred from words or object names.
pub fn english_name(name: &str) -> Option<&'static str> {
    NAMES
        .iter()
        .find_map(|(russian, english)| (*russian == name).then_some(*english))
}

const NAMES: &[(&str, &str)] = &[
    ("АвтоОтступ", "AutoIndent"),
    ("АвтоПозицияРесурсов", "ResourcesAutoPosition"),
    ("БыстрыйВыбор", "QuickChoice"),
    ("ВариантИспользованияГруппировки", "GroupUseVariant"),
    ("ВертикальноеПоложение", "VerticalAlign"),
    ("ВертикальноеПоложениеКартинки", "PictureVerticalAlign"),
    (
        "ВертикальноеРасположениеОбщихИтогов",
        "VerticalOverallPlacement",
    ),
    ("ВесовойКоэффициентШирины", "WidthWeightFactor"),
    ("Видимость", "Visible"),
    ("ВыборГруппИЭлементов", "ChoiceFoldersAndItems"),
    ("ВыводитьЗаголовок", "TitleOutput"),
    ("ВыводитьКартинку", "PictureOutput"),
    ("ВыводитьОтбор", "FilterOutput"),
    ("ВыводитьПараметрыДанных", "DataParametersOutput"),
    ("ВыделятьОтрицательные", "MarkNegatives"),
    ("ГоризонтальноеПоложение", "HorizontalAlign"),
    ("ГоризонтальноеПоложениеКартинки", "PictureHorizontalAlign"),
    (
        "ГоризонтальноеРасположениеОбщихИтогов",
        "HorizontalOverallPlacement",
    ),
    ("Доступность", "Enabled"),
    ("Заголовок", "Title"),
    ("Картинка", "Picture"),
    ("КоличествоЗаписей", "RecordsCount"),
    ("ЛинииТрендаВДиаграмме", "ChartTrendlines"),
    ("МакетОформления", "AppearanceTemplate"),
    ("МаксимальнаяВысота", "MaxHeight"),
    ("МаксимальнаяШирина", "MaxWidth"),
    ("Маска", "Mask"),
    ("МинимальнаяВысота", "MinHeight"),
    ("МинимальнаяШирина", "MinWidth"),
    ("ОбъединятьПоВертикали", "VerticalMerge"),
    ("ОбъединятьПоГоризонтали", "HorizontalMerge"),
    ("ОриентацияТекста", "TextOrientation"),
    ("ОтметкаНезаполненного", "MarkIncomplete"),
    ("Отображать", "Show"),
    (
        "ОтображатьГрафическоеПредставлениеДанныхВЛегендеДиаграммы",
        "ShowGraphicalDataRepresentationInChartLegend",
    ),
    ("Отступ", "Indent"),
    ("ПараметрыВыбора", "ChoiceParameters"),
    (
        "ПоложениеТекстаОтносительноКартинки",
        "TextPositionRelativeToPicture",
    ),
    ("ПроцентЗаписей", "RecordsPercent"),
    ("РазмерКартинки", "PictureSize"),
    ("Размещение", "Placement"),
    ("РасположениеГруппировки", "GroupPlacement"),
    ("РасположениеИтогов", "TotalsPlacement"),
    ("РасположениеОбщихИтогов", "OverallsPlacement"),
    ("РасположениеПолейГруппировки", "GroupFieldsPlacement"),
    ("РасположениеРеквизитов", "AttributePlacement"),
    ("РасположениеРесурсов", "ResourcePlacement"),
    ("РастягиватьПоГоризонтали", "HorizontalStretch"),
    ("Расшифровка", "Details"),
    ("РежимИзмененияРазмераКолонки", "ColumnSizeChangeMode"),
    ("СвязиПараметровВыбора", "ChoiceParameterLinks"),
    ("СвязьПоТипу", "TypeLink"),
    ("СтильГраницы", "BorderStyle"),
    ("Текст", "Text"),
    (
        "ТипГрафическогоПредставленияВДиаграмме",
        "GraphicalRepresentationTypeInChart",
    ),
    ("ТипДиаграммы", "ChartType"),
    ("ТипЗаголовкаПолей", "FieldsTitleType"),
    ("ТипМакета", "TemplateType"),
    ("ТолькоПросмотр", "ReadOnly"),
    ("ФиксацияСверху", "FixedTop"),
    ("ФиксацияСлева", "FixedLeft"),
    ("ФормаВыбора", "ChoiceForm"),
    ("Формат", "Format"),
    ("ФорматРедактирования", "EditFormat"),
    ("ЦветВДиаграмме", "ColorInChart"),
    ("ЦветГраницы", "BorderColor"),
    ("ЦветТекста", "TextColor"),
    ("ЦветФона", "BackColor"),
    ("Шрифт", "Font"),
];
