#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormChildItemKind {
    UsualGroup,
    Other,
}

impl FormChildItemKind {
    fn from_xml_tag(tag: &str) -> Self {
        match tag {
            "UsualGroup" => Self::UsualGroup,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormChildItemRepresentation {
    WeakSeparation,
    Other,
}

impl FormChildItemRepresentation {
    fn from_xml_value(value: &str) -> Self {
        match value {
            "WeakSeparation" => Self::WeakSeparation,
            _ => Self::Other,
        }
    }
}

pub(crate) fn form_child_item_representation_is_default(tag: &str, value: &str) -> bool {
    matches!(
        (
            FormChildItemKind::from_xml_tag(tag),
            FormChildItemRepresentation::from_xml_value(value),
        ),
        (
            FormChildItemKind::UsualGroup,
            FormChildItemRepresentation::WeakSeparation,
        )
    )
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormUsualGroupHeaderXmlProperty {
    Title,
    TitleTextColor,
    TitleFont,
    ToolTip,
    ToolTipRepresentation,
}

pub(crate) const FORM_USUAL_GROUP_HEADER_XML_ORDER: &[FormUsualGroupHeaderXmlProperty] = &[
    FormUsualGroupHeaderXmlProperty::Title,
    FormUsualGroupHeaderXmlProperty::TitleTextColor,
    FormUsualGroupHeaderXmlProperty::TitleFont,
    FormUsualGroupHeaderXmlProperty::ToolTip,
    FormUsualGroupHeaderXmlProperty::ToolTipRepresentation,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormDecorationHeaderXmlProperty {
    Title,
    ToolTip,
    ToolTipRepresentation,
    GroupHorizontalAlign,
    GroupVerticalAlign,
}

pub(crate) const FORM_DECORATION_HEADER_XML_ORDER: &[FormDecorationHeaderXmlProperty] = &[
    FormDecorationHeaderXmlProperty::Title,
    FormDecorationHeaderXmlProperty::ToolTip,
    FormDecorationHeaderXmlProperty::ToolTipRepresentation,
    FormDecorationHeaderXmlProperty::GroupHorizontalAlign,
    FormDecorationHeaderXmlProperty::GroupVerticalAlign,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormExtendedTooltipXmlProperty {
    Width,
    AutoMaxWidth,
    MaxWidth,
    Height,
    AutoMaxHeight,
    HorizontalStretch,
    VerticalStretch,
    TextColor,
    Font,
    Title,
    GroupHorizontalAlign,
    VerticalAlign,
    Events,
}

pub(crate) const FORM_EXTENDED_TOOLTIP_XML_ORDER: &[FormExtendedTooltipXmlProperty] = &[
    FormExtendedTooltipXmlProperty::Width,
    FormExtendedTooltipXmlProperty::AutoMaxWidth,
    FormExtendedTooltipXmlProperty::MaxWidth,
    FormExtendedTooltipXmlProperty::Height,
    FormExtendedTooltipXmlProperty::AutoMaxHeight,
    FormExtendedTooltipXmlProperty::HorizontalStretch,
    FormExtendedTooltipXmlProperty::VerticalStretch,
    FormExtendedTooltipXmlProperty::TextColor,
    FormExtendedTooltipXmlProperty::Font,
    FormExtendedTooltipXmlProperty::Title,
    FormExtendedTooltipXmlProperty::GroupHorizontalAlign,
    FormExtendedTooltipXmlProperty::VerticalAlign,
    FormExtendedTooltipXmlProperty::Events,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormExtendedTooltipSchema {
    width_slot: usize,
    height_slot: usize,
    horizontal_stretch_slot: usize,
    vertical_stretch_slot: usize,
    text_color_slot: usize,
    font_slot: usize,
    auto_max_width_slot: usize,
    max_width_slot: usize,
    auto_max_height_slot: usize,
    group_horizontal_align_slot: usize,
    vertical_align_option_slot: usize,
    title_values_slot: usize,
    title_formatted_slot: usize,
}

impl FormExtendedTooltipSchema {
    pub(crate) const OPTIONS_SLOT: usize = 18;
    pub(crate) const TITLE_SLOT: usize = 22;
    pub(crate) const EVENT_OPTION_SLOT: usize = 5;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        direct_discriminator: Option<&str>,
        options: &[&str],
        title: &[&str],
        event_fields: &[&str],
    ) -> Option<Self> {
        if !matches!(
            (
                wrapper,
                field_count,
                direct_discriminator,
                options.len(),
                options.first().map(|field| field.trim()),
                title.len(),
                title.first().map(|field| field.trim()),
                title.get(2).map(|field| field.trim()),
            ),
            (
                "12",
                34,
                Some("0"),
                9,
                Some("5"),
                3,
                Some("1"),
                Some("0" | "1")
            )
        ) || !Self::event_fields_are_exact(event_fields)
        {
            return None;
        }
        Some(Self {
            width_slot: 10,
            height_slot: 11,
            horizontal_stretch_slot: 12,
            vertical_stretch_slot: 13,
            text_color_slot: 14,
            font_slot: 15,
            auto_max_width_slot: 25,
            max_width_slot: 26,
            auto_max_height_slot: 28,
            group_horizontal_align_slot: 30,
            vertical_align_option_slot: 3,
            title_values_slot: 1,
            title_formatted_slot: 2,
        })
    }

    fn event_fields_are_exact(fields: &[&str]) -> bool {
        (fields.len() == 3
            && fields.first().map(|field| field.trim()) == Some("0")
            && fields.get(1).map(|field| field.trim()) == Some("1")
            && fields.get(2).map(|field| field.trim()) == Some("0"))
            || (fields.len() == 8
                && fields.first().map(|field| field.trim()) == Some("1")
                && fields.get(3).map(|field| field.trim()) == Some("1")
                && fields.get(4).map(|field| field.trim()) == Some("0")
                && fields.get(6).map(|field| field.trim()) == Some("0")
                && fields.get(7).map(|field| field.trim()) == Some("1")
                && fields.get(1).map(|field| field.trim())
                    == fields.get(5).map(|field| field.trim()))
    }

    pub(crate) const fn width_slot(self) -> usize {
        self.width_slot
    }

    pub(crate) const fn height_slot(self) -> usize {
        self.height_slot
    }

    pub(crate) const fn horizontal_stretch_slot(self) -> usize {
        self.horizontal_stretch_slot
    }

    pub(crate) const fn vertical_stretch_slot(self) -> usize {
        self.vertical_stretch_slot
    }

    pub(crate) const fn text_color_slot(self) -> usize {
        self.text_color_slot
    }

    pub(crate) const fn font_slot(self) -> usize {
        self.font_slot
    }

    pub(crate) const fn auto_max_width_slot(self) -> usize {
        self.auto_max_width_slot
    }

    pub(crate) const fn max_width_slot(self) -> usize {
        self.max_width_slot
    }

    pub(crate) const fn auto_max_height_slot(self) -> usize {
        self.auto_max_height_slot
    }

    pub(crate) const fn group_horizontal_align_slot(self) -> usize {
        self.group_horizontal_align_slot
    }

    pub(crate) const fn vertical_align_option_slot(self) -> usize {
        self.vertical_align_option_slot
    }

    pub(crate) const fn title_values_slot(self) -> usize {
        self.title_values_slot
    }

    pub(crate) const fn title_formatted_slot(self) -> usize {
        self.title_formatted_slot
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormDecorationHeaderSchema {
    tooltip_slot: usize,
    tooltip_representation_slot: usize,
}

impl FormDecorationHeaderSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
    ) -> Option<Self> {
        match (wrapper, field_count, item_tag, direct_discriminator) {
            ("12", 36, "LabelDecoration", Some("0"))
            | ("12", 36, "PictureDecoration", Some("1")) => Some(Self {
                tooltip_slot: 8,
                tooltip_representation_slot: 24,
            }),
            _ => None,
        }
    }

    pub(crate) const fn tooltip_slot(self) -> usize {
        self.tooltip_slot
    }

    pub(crate) const fn tooltip_representation_slot(self) -> usize {
        self.tooltip_representation_slot
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormLabelDecorationSchema {
    width_slot: usize,
    height_slot: usize,
    horizontal_stretch_slot: usize,
    vertical_stretch_slot: usize,
    skip_on_input_slot: usize,
    auto_max_width_slot: usize,
    max_width_slot: usize,
    auto_max_height_slot: usize,
    max_height_slot: usize,
    group_horizontal_align_slot: usize,
    group_vertical_align_slot: usize,
    horizontal_align_option_slot: usize,
    vertical_align_option_slot: usize,
    title_height_option_slot: usize,
}

impl FormLabelDecorationSchema {
    pub(crate) const OPTIONS_SLOT: usize = 18;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        match (
            wrapper,
            field_count,
            item_tag,
            direct_discriminator,
            options.len(),
            options.first().map(|field| field.trim()),
        ) {
            ("12", 36, "LabelDecoration", Some("0"), 9, Some("5")) => Some(Self {
                width_slot: 10,
                height_slot: 11,
                horizontal_stretch_slot: 12,
                vertical_stretch_slot: 13,
                skip_on_input_slot: 22,
                auto_max_width_slot: 27,
                max_width_slot: 28,
                auto_max_height_slot: 30,
                max_height_slot: 31,
                group_horizontal_align_slot: 32,
                group_vertical_align_slot: 33,
                horizontal_align_option_slot: 2,
                vertical_align_option_slot: 3,
                title_height_option_slot: 4,
            }),
            _ => None,
        }
    }

    pub(crate) const fn group_horizontal_align_slot(self) -> usize {
        self.group_horizontal_align_slot
    }

    pub(crate) const fn group_vertical_align_slot(self) -> usize {
        self.group_vertical_align_slot
    }

    pub(crate) const fn horizontal_align_option_slot(self) -> usize {
        self.horizontal_align_option_slot
    }

    pub(crate) const fn vertical_align_option_slot(self) -> usize {
        self.vertical_align_option_slot
    }

    pub(crate) fn alignment(
        self,
        fields: &[&str],
        options: &[&str],
    ) -> FormLabelDecorationAlignment {
        FormLabelDecorationAlignment {
            group_vertical_align: fields
                .get(self.group_vertical_align_slot())
                .and_then(|field| match field.trim() {
                    "1" => Some("Center"),
                    "2" => Some("Bottom"),
                    _ => None,
                }),
            horizontal_align: options
                .get(self.horizontal_align_option_slot())
                .and_then(|field| match field.trim() {
                    "1" => Some("Center"),
                    "2" => Some("Right"),
                    "3" => Some("Auto"),
                    _ => None,
                }),
            vertical_align: options
                .get(self.vertical_align_option_slot())
                .and_then(|field| match field.trim() {
                    "0" => Some("Top"),
                    "1" => Some("Center"),
                    "2" => Some("Bottom"),
                    _ => None,
                }),
        }
    }

    pub(crate) fn geometry(self, fields: &[&str]) -> FormLabelDecorationGeometry {
        FormLabelDecorationGeometry {
            width: Self::non_zero_u32(fields, self.width_slot),
            auto_max_width: Self::false_or_omit(fields, self.auto_max_width_slot),
            max_width: Self::non_zero_u32(fields, self.max_width_slot),
            height: Self::non_zero_u32(fields, self.height_slot),
            auto_max_height: Self::false_or_omit(fields, self.auto_max_height_slot),
            max_height: Self::non_zero_u32(fields, self.max_height_slot),
            horizontal_stretch: Self::stretch(fields, self.horizontal_stretch_slot),
            vertical_stretch: Self::stretch(fields, self.vertical_stretch_slot),
        }
    }

    pub(crate) fn visual_tail(self, options: &[&str]) -> FormLabelDecorationVisualTail {
        FormLabelDecorationVisualTail {
            title_height: Self::non_zero_u32(options, self.title_height_option_slot),
        }
    }

    pub(crate) fn skip_on_input(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.skip_on_input_slot)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }

    fn non_zero_u32(fields: &[&str], slot: usize) -> Option<String> {
        let value = fields.get(slot)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }

    fn false_or_omit(fields: &[&str], slot: usize) -> Option<bool> {
        (fields.get(slot)?.trim() == "0").then_some(false)
    }

    fn stretch(fields: &[&str], slot: usize) -> Option<bool> {
        match fields.get(slot)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormLabelDecorationVisualTail {
    title_height: Option<String>,
}

impl FormLabelDecorationVisualTail {
    pub(crate) fn title_height(&self) -> Option<&str> {
        self.title_height.as_deref()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormLabelDecorationAlignment {
    group_vertical_align: Option<&'static str>,
    horizontal_align: Option<&'static str>,
    vertical_align: Option<&'static str>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormLabelDecorationGeometry {
    width: Option<String>,
    auto_max_width: Option<bool>,
    max_width: Option<String>,
    height: Option<String>,
    auto_max_height: Option<bool>,
    max_height: Option<String>,
    horizontal_stretch: Option<bool>,
    vertical_stretch: Option<bool>,
}

impl FormLabelDecorationGeometry {
    pub(crate) fn width(&self) -> Option<&str> {
        self.width.as_deref()
    }

    pub(crate) const fn auto_max_width(&self) -> Option<bool> {
        self.auto_max_width
    }

    pub(crate) fn max_width(&self) -> Option<&str> {
        self.max_width.as_deref()
    }

    pub(crate) fn height(&self) -> Option<&str> {
        self.height.as_deref()
    }

    pub(crate) const fn auto_max_height(&self) -> Option<bool> {
        self.auto_max_height
    }

    pub(crate) fn max_height(&self) -> Option<&str> {
        self.max_height.as_deref()
    }

    pub(crate) const fn horizontal_stretch(&self) -> Option<bool> {
        self.horizontal_stretch
    }

    pub(crate) const fn vertical_stretch(&self) -> Option<bool> {
        self.vertical_stretch
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormChildItemAlignment {
    Horizontal(&'static str),
    LabelDecoration(FormLabelDecorationAlignment),
}

impl FormChildItemAlignment {
    pub(crate) const fn horizontal_align(self) -> Option<&'static str> {
        match self {
            Self::Horizontal(value) => Some(value),
            Self::LabelDecoration(alignment) => alignment.horizontal_align,
        }
    }

    pub(crate) const fn group_vertical_align(self) -> Option<&'static str> {
        match self {
            Self::Horizontal(_) => None,
            Self::LabelDecoration(alignment) => alignment.group_vertical_align,
        }
    }

    pub(crate) const fn vertical_align(self) -> Option<&'static str> {
        match self {
            Self::Horizontal(_) => None,
            Self::LabelDecoration(alignment) => alignment.vertical_align,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormLabelDecorationAlignmentTailXmlProperty {
    HorizontalAlign,
    VerticalAlign,
}

pub(crate) const FORM_LABEL_DECORATION_ALIGNMENT_TAIL_XML_ORDER:
    &[FormLabelDecorationAlignmentTailXmlProperty] = &[
    FormLabelDecorationAlignmentTailXmlProperty::HorizontalAlign,
    FormLabelDecorationAlignmentTailXmlProperty::VerticalAlign,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormLabelDecorationVisualTailXmlProperty {
    TitleHeight,
}

pub(crate) const FORM_LABEL_DECORATION_VISUAL_TAIL_XML_ORDER:
    &[FormLabelDecorationVisualTailXmlProperty] =
    &[FormLabelDecorationVisualTailXmlProperty::TitleHeight];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormLabelDecorationGeometryXmlProperty {
    Width,
    AutoMaxWidth,
    MaxWidth,
    Height,
    AutoMaxHeight,
    MaxHeight,
    HorizontalStretch,
    VerticalStretch,
}

pub(crate) const FORM_LABEL_DECORATION_GEOMETRY_XML_ORDER:
    &[FormLabelDecorationGeometryXmlProperty] = &[
    FormLabelDecorationGeometryXmlProperty::Width,
    FormLabelDecorationGeometryXmlProperty::AutoMaxWidth,
    FormLabelDecorationGeometryXmlProperty::MaxWidth,
    FormLabelDecorationGeometryXmlProperty::Height,
    FormLabelDecorationGeometryXmlProperty::AutoMaxHeight,
    FormLabelDecorationGeometryXmlProperty::MaxHeight,
    FormLabelDecorationGeometryXmlProperty::HorizontalStretch,
    FormLabelDecorationGeometryXmlProperty::VerticalStretch,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormCheckBoxFieldSchema {
    top_level_offset: usize,
}

impl FormCheckBoxFieldSchema {
    pub(crate) fn top_level_offset_for_raw_layout(
        wrapper: &str,
        field_count: usize,
    ) -> Option<usize> {
        match (wrapper, field_count) {
            ("37", 59) => Some(0),
            ("37", 60) => Some(1),
            _ => None,
        }
    }

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        let top_level_offset = Self::top_level_offset_for_raw_layout(wrapper, field_count)?;
        if direct_discriminator != Some("3")
            || options.len() != 13
            || options.first().map(|field| field.trim()) != Some("11")
        {
            return None;
        }
        Some(Self { top_level_offset })
    }

    pub(crate) const fn options_slot(self) -> usize {
        39 + self.top_level_offset
    }

    pub(crate) const fn tooltip_slot(self) -> usize {
        10 + self.top_level_offset
    }

    pub(crate) fn horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields
            .get(23 + self.top_level_offset)
            .map(|field| field.trim())?
        {
            "0" => Some("Left"),
            "1" => Some("Center"),
            "3" => None,
            _ => None,
        }
    }

    pub(crate) fn check_box_type(self, options: &[&str]) -> Option<&'static str> {
        match (
            options.get(1).map(|field| field.trim()),
            options.get(12).map(|field| field.trim()),
        ) {
            (Some("1"), Some("0")) => None,
            (Some("0"), Some("0")) => Some("Auto"),
            (Some("0"), Some("1")) => Some("CheckBox"),
            (Some("0"), Some("2")) => Some("Tumbler"),
            (Some("0"), Some("3")) => Some("Switcher"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormChildItemVisibleSchema {
    slot: usize,
}

impl FormChildItemVisibleSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        top_level_offset: usize,
    ) -> Option<Self> {
        let slot = match (wrapper, item_tag, direct_discriminator) {
            ("22", "CommandBar", Some("0"))
            | ("22", "Popup", Some("1"))
            | ("22", "ColumnGroup", Some("2"))
            | ("22", "Pages", Some("3"))
            | ("22", "Page", Some("4"))
            | ("22", "UsualGroup", Some("5"))
            | ("22", "ButtonGroup", Some("6"))
                if field_count >= 30 && (field_count - 30) % 2 == 0 =>
            {
                field_count.checked_sub(8)?
            }
            ("12", "LabelDecoration", Some("0")) | ("12", "PictureDecoration", Some("1"))
                if field_count == 36 =>
            {
                21
            }
            ("31", "Button", _) if field_count == 52 => 26,
            ("37", "LabelField", Some("1"))
            | ("37", "InputField", Some("2"))
            | ("37", "CheckBoxField", Some("3"))
            | ("37", "PictureField", Some("4"))
            | ("37", "RadioButtonField", Some("5"))
            | ("37", "SpreadSheetDocumentField", Some("6"))
                if matches!((field_count, top_level_offset), (59, 0) | (60, 1)) =>
            {
                43 + top_level_offset
            }
            ("55", "Table", _) if field_count >= 99 && (field_count - 99) % 2 == 0 => {
                field_count.checked_sub(35)?
            }
            _ => return None,
        };
        Some(Self { slot })
    }

    pub(crate) fn visible(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.slot)?.trim() == "0").then_some(false)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormTooltipRepresentationItemKind {
    UsualGroup,
    LabelDecoration,
    PictureDecoration,
    LabelField,
    InputField,
    CheckBoxField,
    PictureField,
    RadioButtonField,
    CalendarField,
    Button,
    Other,
}

impl FormTooltipRepresentationItemKind {
    fn from_xml_tag(tag: &str) -> Self {
        match tag {
            "UsualGroup" => Self::UsualGroup,
            "LabelDecoration" => Self::LabelDecoration,
            "PictureDecoration" => Self::PictureDecoration,
            "LabelField" => Self::LabelField,
            "InputField" => Self::InputField,
            "CheckBoxField" => Self::CheckBoxField,
            "PictureField" => Self::PictureField,
            "RadioButtonField" => Self::RadioButtonField,
            "CalendarField" => Self::CalendarField,
            "Button" => Self::Button,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTooltipRepresentationXmlOrder {
    UsualGroupHeader,
    DecorationHeader,
    FieldProperties,
    AfterTitle,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormTooltipRepresentationSchema {
    slot: usize,
}

impl FormTooltipRepresentationSchema {
    pub(crate) const fn slot(self) -> usize {
        self.slot
    }
}

pub(crate) fn form_tooltip_representation_schema(
    wrapper: &str,
    field_count: usize,
    item_tag: &str,
    direct_discriminator: Option<&str>,
) -> Option<FormTooltipRepresentationSchema> {
    if let Some(schema) = FormDecorationHeaderSchema::from_raw_layout(
        wrapper,
        field_count,
        item_tag,
        direct_discriminator,
    ) {
        return Some(FormTooltipRepresentationSchema {
            slot: schema.tooltip_representation_slot(),
        });
    }
    let item_kind = FormTooltipRepresentationItemKind::from_xml_tag(item_tag);
    let slot = match (wrapper, field_count, item_kind, direct_discriminator) {
        ("22", 30, FormTooltipRepresentationItemKind::UsualGroup, Some("5")) => 23,
        ("22", 32, FormTooltipRepresentationItemKind::UsualGroup, Some("5")) => 25,
        ("22", 34, FormTooltipRepresentationItemKind::UsualGroup, Some("5")) => 27,
        ("22", 36, FormTooltipRepresentationItemKind::UsualGroup, Some("5")) => 29,
        ("37", 59, FormTooltipRepresentationItemKind::LabelField, Some("1"))
        | ("37", 59, FormTooltipRepresentationItemKind::InputField, Some("2"))
        | ("37", 59, FormTooltipRepresentationItemKind::CheckBoxField, Some("3"))
        | ("37", 59, FormTooltipRepresentationItemKind::PictureField, Some("4"))
        | ("37", 59, FormTooltipRepresentationItemKind::RadioButtonField, Some("5"))
        | ("37", 59, FormTooltipRepresentationItemKind::CalendarField, Some("8")) => 50,
        ("31", 52, FormTooltipRepresentationItemKind::Button, _) => 30,
        _ => return None,
    };
    Some(FormTooltipRepresentationSchema { slot })
}

pub(crate) fn form_tooltip_representation_xml_order(
    item_tag: &str,
) -> Option<FormTooltipRepresentationXmlOrder> {
    match FormTooltipRepresentationItemKind::from_xml_tag(item_tag) {
        FormTooltipRepresentationItemKind::UsualGroup => {
            Some(FormTooltipRepresentationXmlOrder::UsualGroupHeader)
        }
        FormTooltipRepresentationItemKind::LabelDecoration
        | FormTooltipRepresentationItemKind::PictureDecoration => {
            Some(FormTooltipRepresentationXmlOrder::DecorationHeader)
        }
        FormTooltipRepresentationItemKind::LabelField
        | FormTooltipRepresentationItemKind::InputField
        | FormTooltipRepresentationItemKind::CheckBoxField
        | FormTooltipRepresentationItemKind::PictureField
        | FormTooltipRepresentationItemKind::RadioButtonField
        | FormTooltipRepresentationItemKind::CalendarField => {
            Some(FormTooltipRepresentationXmlOrder::FieldProperties)
        }
        FormTooltipRepresentationItemKind::Button => {
            Some(FormTooltipRepresentationXmlOrder::AfterTitle)
        }
        FormTooltipRepresentationItemKind::Other => None,
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormTooltipRepresentation {
    Omit,
    None,
    Balloon,
    Button,
    ShowAuto,
    ShowTop,
    ShowLeft,
    ShowBottom,
    ShowRight,
}

impl FormTooltipRepresentation {
    fn from_raw_scalar(value: &str) -> Option<Self> {
        match value {
            "0" => Some(Self::Omit),
            "1" => Some(Self::None),
            "2" => Some(Self::Balloon),
            "3" => Some(Self::Button),
            "4" => Some(Self::ShowAuto),
            "5" => Some(Self::ShowTop),
            "6" => Some(Self::ShowLeft),
            "7" => Some(Self::ShowBottom),
            "8" => Some(Self::ShowRight),
            _ => None,
        }
    }

    const fn xml_value(self) -> Option<&'static str> {
        match self {
            Self::Omit => None,
            Self::None => Some("None"),
            Self::Balloon => Some("Balloon"),
            Self::Button => Some("Button"),
            Self::ShowAuto => Some("ShowAuto"),
            Self::ShowTop => Some("ShowTop"),
            Self::ShowLeft => Some("ShowLeft"),
            Self::ShowBottom => Some("ShowBottom"),
            Self::ShowRight => Some("ShowRight"),
        }
    }
}

pub(crate) fn decode_form_tooltip_representation(value: &str) -> Option<&'static str> {
    FormTooltipRepresentation::from_raw_scalar(value)?.xml_value()
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormInputFieldXmlProperty {
    DropListButton,
    ChoiceButton,
    ChoiceButtonRepresentation,
    ClearButton,
    SpinButton,
    OpenButton,
    CreateButton,
    ChoiceListButton,
}

pub(crate) const FORM_INPUT_FIELD_BUTTON_XML_ORDER: &[FormInputFieldXmlProperty] = &[
    FormInputFieldXmlProperty::DropListButton,
    FormInputFieldXmlProperty::ChoiceButton,
    FormInputFieldXmlProperty::ChoiceButtonRepresentation,
    FormInputFieldXmlProperty::ClearButton,
    FormInputFieldXmlProperty::SpinButton,
    FormInputFieldXmlProperty::OpenButton,
    FormInputFieldXmlProperty::CreateButton,
    FormInputFieldXmlProperty::ChoiceListButton,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableXmlProperty {
    Representation,
    Visible,
    CommandBarLocation,
    DefaultItem,
    UseAlternationRowColor,
    InitialTreeView,
    AutoMarkIncomplete,
    SkipOnInput,
    ReadOnly,
    ChangeRowSet,
    Height,
    AutoMaxHeight,
    HeightInTableRows,
    ChangeRowOrder,
    AutoMaxWidth,
    RowInputMode,
    AutoInsertNewRow,
    EnableStartDrag,
    EnableDrag,
    FileDragMode,
    DataPath,
    RowPictureDataPath,
    RowsPicture,
    Title,
    CommandSet,
    AutoRefresh,
    AutoRefreshPeriod,
    Period,
    ChoiceFoldersAndItems,
    RestoreCurrentRow,
    TopLevelParent,
    ShowRoot,
    AllowRootChoice,
    UpdateOnDataChange,
    UserSettingsGroup,
    AllowGettingCurrentRowURL,
}

pub(crate) const FORM_TABLE_XML_ORDER: &[FormTableXmlProperty] = &[
    FormTableXmlProperty::Representation,
    FormTableXmlProperty::Visible,
    FormTableXmlProperty::CommandBarLocation,
    FormTableXmlProperty::DefaultItem,
    FormTableXmlProperty::UseAlternationRowColor,
    FormTableXmlProperty::InitialTreeView,
    FormTableXmlProperty::AutoMarkIncomplete,
    FormTableXmlProperty::SkipOnInput,
    FormTableXmlProperty::ReadOnly,
    FormTableXmlProperty::ChangeRowSet,
    FormTableXmlProperty::Height,
    FormTableXmlProperty::AutoMaxHeight,
    FormTableXmlProperty::HeightInTableRows,
    FormTableXmlProperty::ChangeRowOrder,
    FormTableXmlProperty::AutoMaxWidth,
    FormTableXmlProperty::RowInputMode,
    FormTableXmlProperty::AutoInsertNewRow,
    FormTableXmlProperty::EnableStartDrag,
    FormTableXmlProperty::EnableDrag,
    FormTableXmlProperty::FileDragMode,
    FormTableXmlProperty::DataPath,
    FormTableXmlProperty::RowPictureDataPath,
    FormTableXmlProperty::RowsPicture,
    FormTableXmlProperty::Title,
    FormTableXmlProperty::CommandSet,
    FormTableXmlProperty::AutoRefresh,
    FormTableXmlProperty::AutoRefreshPeriod,
    FormTableXmlProperty::Period,
    FormTableXmlProperty::ChoiceFoldersAndItems,
    FormTableXmlProperty::RestoreCurrentRow,
    FormTableXmlProperty::TopLevelParent,
    FormTableXmlProperty::ShowRoot,
    FormTableXmlProperty::AllowRootChoice,
    FormTableXmlProperty::UpdateOnDataChange,
    FormTableXmlProperty::UserSettingsGroup,
    FormTableXmlProperty::AllowGettingCurrentRowURL,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTablePropertyBagKey {
    AutoRefresh,
    AutoRefreshPeriod,
    Period,
    ChoiceFoldersAndItems,
    UseAlternationRowColor,
    RowFilter,
    DefaultItem,
    RestoreCurrentRow,
    UpdateOnDataChange,
    TopLevelParent,
    UserSettingsGroup,
    RowPictureDataPath,
    AllowGettingCurrentRowUrl,
}

impl FormTablePropertyBagKey {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::AutoRefresh => "5",
            Self::AutoRefreshPeriod => "6",
            Self::Period => "7",
            Self::ChoiceFoldersAndItems => "8",
            Self::UseAlternationRowColor => "9",
            Self::RowFilter => "10",
            Self::DefaultItem => "11",
            Self::RestoreCurrentRow => "12",
            Self::UpdateOnDataChange => "14",
            Self::TopLevelParent => "15",
            Self::UserSettingsGroup => "16",
            Self::RowPictureDataPath => "19",
            Self::AllowGettingCurrentRowUrl => "20",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableOrdinaryTailKey {
    RowFilter,
}

impl FormTableOrdinaryTailKey {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::RowFilter => "13",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormInputFieldExtendedOptionSlot {
    Width,
    Height,
    HorizontalStretch,
    VerticalStretch,
    Wrap,
    PasswordMode,
    MultiLine,
    ChoiceListButton,
    ChoiceButton,
    ClearButton,
    SpinButton,
    OpenButton,
    ListChoiceMode,
    QuickChoice,
    AutoCellHeight,
    ChoiceFoldersAndItems,
    AutoChoiceIncomplete,
    AutoMarkIncomplete,
    ChooseType,
    Format,
    EditFormat,
    Font,
    TextEdit,
    CreateButton,
    ChoiceButtonRepresentation,
    DropListButton,
    AutoMaxWidth,
    MaxWidth,
    AutoMaxHeight,
    MaxHeight,
}

impl FormInputFieldExtendedOptionSlot {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Width => 2,
            Self::Height => 3,
            Self::HorizontalStretch => 4,
            Self::VerticalStretch => 5,
            Self::Wrap => 6,
            Self::PasswordMode => 7,
            Self::MultiLine => 8,
            Self::ChoiceListButton => 11,
            Self::ChoiceButton => 12,
            Self::ClearButton => 13,
            Self::SpinButton => 14,
            Self::OpenButton => 15,
            Self::ListChoiceMode => 19,
            Self::QuickChoice => 23,
            Self::ChoiceFoldersAndItems => 24,
            Self::AutoCellHeight => 28,
            Self::AutoChoiceIncomplete => 28,
            Self::Format => 29,
            Self::EditFormat => 30,
            Self::AutoMarkIncomplete => 31,
            Self::ChooseType => 32,
            Self::Font => 40,
            Self::TextEdit => 41,
            Self::CreateButton => 45,
            Self::ChoiceButtonRepresentation => 46,
            Self::DropListButton => 47,
            Self::AutoMaxWidth => 49,
            Self::MaxWidth => 50,
            Self::AutoMaxHeight => 52,
            Self::MaxHeight => 53,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFieldTopLevelSlot {
    DefaultItem,
    TitleFont,
}

impl FormFieldTopLevelSlot {
    pub(crate) const fn index(self, top_level_offset: usize) -> usize {
        match self {
            Self::DefaultItem => 16 + top_level_offset,
            Self::TitleFont => 32 + top_level_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormLabelFieldOptionSlot {
    Width,
    Height,
    HorizontalStretch,
    Format,
    MaxWidth,
    TextColor,
    Font,
    AutoMaxWidth,
    AutoMaxHeight,
}

impl FormLabelFieldOptionSlot {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Width => 1,
            Self::Height => 2,
            Self::HorizontalStretch => 3,
            Self::Format => 6,
            Self::MaxWidth => 7,
            Self::TextColor => 8,
            Self::Font => 10,
            Self::AutoMaxWidth => 15,
            Self::AutoMaxHeight => 18,
        }
    }
}
