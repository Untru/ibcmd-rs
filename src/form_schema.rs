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

// Platform type ID used by serialized Form column patterns for a DCS filter.
const FORM_DATA_COMPOSITION_FILTER_TYPE_UUID: &str = "f6841c6b-6c71-4c82-ae9e-d08b49db326c";

pub(crate) fn form_attribute_column_builtin_type_reference(type_id: &str) -> Option<&'static str> {
    type_id
        .eq_ignore_ascii_case(FORM_DATA_COMPOSITION_FILTER_TYPE_UUID)
        .then_some("dcsset:Filter")
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormAttributeAdditionalColumnsBindingKind {
    Numeric,
    MetadataReference,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormAttributeAdditionalColumnsGroupSchema {
    column_count: usize,
    binding_kind: FormAttributeAdditionalColumnsBindingKind,
}

impl FormAttributeAdditionalColumnsGroupSchema {
    pub(crate) fn from_raw_layout(
        fields: &[&str],
        target: &[&str],
        owner: &[&str],
        binding: &[&str],
    ) -> Option<Self> {
        let column_count = fields.get(2)?.trim().parse::<usize>().ok()?;
        if fields.len() != 3 + column_count
            || fields.first().map(|field| field.trim()) != Some("0")
            || target.len() != 3
            || target.first().map(|field| field.trim()) != Some("2")
            || owner.len() != 1
            || owner.first()?.trim().is_empty()
        {
            return None;
        }
        let binding_kind = match binding {
            [number] if number.trim().parse::<u64>().is_ok() => {
                FormAttributeAdditionalColumnsBindingKind::Numeric
            }
            [prefix, uuid] if prefix.trim() == "0" && !uuid.trim().is_empty() => {
                FormAttributeAdditionalColumnsBindingKind::MetadataReference
            }
            _ => return None,
        };
        Some(Self {
            column_count,
            binding_kind,
        })
    }

    pub(crate) const fn column_count(self) -> usize {
        self.column_count
    }

    pub(crate) const fn binding_kind(self) -> FormAttributeAdditionalColumnsBindingKind {
        self.binding_kind
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormAttributeColumnSchema;

impl FormAttributeColumnSchema {
    pub(crate) fn from_raw_layout(fields: &[&str]) -> Option<Self> {
        (fields.len() == 10 && fields.first().map(|field| field.trim()) == Some("5"))
            .then_some(Self)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormPageXmlProperty {
    EnableContentChange,
    Title,
    ToolTip,
    ToolTipRepresentation,
    Picture,
    HorizontalStretch,
    VerticalStretch,
    Group,
    HorizontalAlign,
    VerticalAlign,
    ChildItemsWidth,
    ShowTitle,
    BackColor,
}

pub(crate) const FORM_PAGE_XML_ORDER: &[FormPageXmlProperty] = &[
    FormPageXmlProperty::EnableContentChange,
    FormPageXmlProperty::Title,
    FormPageXmlProperty::ToolTip,
    FormPageXmlProperty::ToolTipRepresentation,
    FormPageXmlProperty::Picture,
    FormPageXmlProperty::HorizontalStretch,
    FormPageXmlProperty::VerticalStretch,
    FormPageXmlProperty::Group,
    FormPageXmlProperty::HorizontalAlign,
    FormPageXmlProperty::VerticalAlign,
    FormPageXmlProperty::ChildItemsWidth,
    FormPageXmlProperty::ShowTitle,
    FormPageXmlProperty::BackColor,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormPopupRepresentation {
    Text,
    Picture,
    PictureAndText,
    Default,
}

impl FormPopupRepresentation {
    fn from_raw_scalar(value: &str) -> Option<Self> {
        match value {
            "0" => Some(Self::Text),
            "1" => Some(Self::Picture),
            "2" => Some(Self::PictureAndText),
            "3" => Some(Self::Default),
            _ => None,
        }
    }

    const fn xml_value(self) -> Option<&'static str> {
        match self {
            Self::Text => Some("Text"),
            Self::Picture => Some("Picture"),
            Self::PictureAndText => Some("PictureAndText"),
            Self::Default => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPopupSchema {
    representation: FormPopupRepresentation,
}

impl FormPopupSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        if wrapper != "22"
            || field_count < 30
            || (field_count - 30) % 2 != 0
            || item_tag != "Popup"
            || direct_discriminator != Some("1")
            || options.len() != 9
            || options.first().map(|field| field.trim()) != Some("7")
            || options.get(3).map(|field| field.trim()) != Some("2")
            || options.get(5).map(|field| field.trim()) != Some("0")
            || options.get(6).map(|field| field.trim()) != Some("0")
        {
            return None;
        }
        Some(Self {
            representation: FormPopupRepresentation::from_raw_scalar(options.get(4)?.trim())?,
        })
    }

    pub(crate) const fn representation(self) -> Option<&'static str> {
        self.representation.xml_value()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPageSchema;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPageProperties {
    enable_content_change: Option<bool>,
    horizontal_stretch: Option<bool>,
    vertical_stretch: Option<bool>,
    group: Option<&'static str>,
    horizontal_align: Option<&'static str>,
    vertical_align: Option<&'static str>,
    child_items_width: Option<&'static str>,
}

impl FormPageSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        (wrapper == "22"
            && field_count >= 30
            && (field_count - 30) % 2 == 0
            && item_tag == "Page"
            && direct_discriminator == Some("4")
            && options.len() == 20
            && options.first().map(|field| field.trim()) == Some("18"))
        .then_some(Self)
    }

    pub(crate) fn properties(self, fields: &[&str], options: &[&str]) -> FormPageProperties {
        let group = match (
            options.get(2).map(|field| field.trim()),
            options.get(16).map(|field| field.trim()),
            options.get(17).map(|field| field.trim()),
        ) {
            (Some("0"), Some("0"), Some("0")) => None,
            (Some("1"), Some("1"), Some("1")) => Some("Horizontal"),
            (Some("1"), Some("2"), Some("2")) => Some("HorizontalIfPossible"),
            (Some("1"), Some("1"), Some("3")) => Some("AlwaysHorizontal"),
            _ => None,
        };
        FormPageProperties {
            enable_content_change: match fields.get(9).map(|field| field.trim()) {
                Some("1") => Some(true),
                _ => None,
            },
            horizontal_stretch: match fields.get(14).map(|field| field.trim()) {
                Some("0") => Some(false),
                Some("1") => Some(true),
                _ => None,
            },
            vertical_stretch: match fields.get(15).map(|field| field.trim()) {
                Some("0") => Some(false),
                Some("1") => Some(true),
                _ => None,
            },
            group,
            horizontal_align: match options.get(12).map(|field| field.trim()) {
                Some("1") => Some("Center"),
                _ => None,
            },
            vertical_align: match options.get(13).map(|field| field.trim()) {
                Some("1") => Some("Center"),
                Some("2") => Some("Bottom"),
                _ => None,
            },
            child_items_width: match options.get(3).map(|field| field.trim()) {
                Some("3") => Some("LeftWidest"),
                Some("5") => Some("LeftNarrowest"),
                _ => None,
            },
        }
    }

    pub(crate) const fn picture_option_slot(self) -> usize {
        1
    }

    pub(crate) fn picture(self, value: &[&str]) -> Option<FormPictureValueSchema> {
        let picture = FormPictureValueSchema::from_raw_layout(value)?;
        matches!(
            picture.kind(),
            FormPictureValueKind::Empty | FormPictureValueKind::Reference
        )
        .then_some(picture)
    }
}

impl FormPageProperties {
    pub(crate) const fn enable_content_change(self) -> Option<bool> {
        self.enable_content_change
    }

    pub(crate) const fn horizontal_stretch(self) -> Option<bool> {
        self.horizontal_stretch
    }

    pub(crate) const fn vertical_stretch(self) -> Option<bool> {
        self.vertical_stretch
    }

    pub(crate) const fn group(self) -> Option<&'static str> {
        self.group
    }

    pub(crate) const fn horizontal_align(self) -> Option<&'static str> {
        self.horizontal_align
    }

    pub(crate) const fn vertical_align(self) -> Option<&'static str> {
        self.vertical_align
    }

    pub(crate) const fn child_items_width(self) -> Option<&'static str> {
        self.child_items_width
    }
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
pub(crate) enum FormUsualGroupXmlAnchor {
    BeforeTitle,
    BeforeGroup,
    BeforeBehavior,
    AfterBehavior,
    AfterRepresentation,
    BeforeExtendedTooltip,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormUsualGroupXmlProperty {
    EnableContentChange,
    GroupVerticalAlign,
    HorizontalAlign,
    VerticalAlign,
    Collapsed,
    ControlRepresentation,
    United,
    ChildItemsWidth,
    ThroughAlign,
}

pub(crate) const FORM_USUAL_GROUP_XML_ORDER: &[FormUsualGroupXmlProperty] = &[
    FormUsualGroupXmlProperty::EnableContentChange,
    FormUsualGroupXmlProperty::GroupVerticalAlign,
    FormUsualGroupXmlProperty::HorizontalAlign,
    FormUsualGroupXmlProperty::VerticalAlign,
    FormUsualGroupXmlProperty::Collapsed,
    FormUsualGroupXmlProperty::ControlRepresentation,
    FormUsualGroupXmlProperty::United,
    FormUsualGroupXmlProperty::ChildItemsWidth,
    FormUsualGroupXmlProperty::ThroughAlign,
];

impl FormUsualGroupXmlProperty {
    pub(crate) const fn anchor(self) -> FormUsualGroupXmlAnchor {
        match self {
            Self::EnableContentChange => FormUsualGroupXmlAnchor::BeforeTitle,
            Self::GroupVerticalAlign => FormUsualGroupXmlAnchor::BeforeGroup,
            Self::HorizontalAlign | Self::VerticalAlign => FormUsualGroupXmlAnchor::BeforeBehavior,
            Self::Collapsed | Self::ControlRepresentation => FormUsualGroupXmlAnchor::AfterBehavior,
            Self::United | Self::ChildItemsWidth => FormUsualGroupXmlAnchor::AfterRepresentation,
            Self::ThroughAlign => FormUsualGroupXmlAnchor::BeforeExtendedTooltip,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormUsualGroupSchema;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormUsualGroupProperties {
    enable_content_change: Option<bool>,
    group_vertical_align: Option<FormUsualGroupGroupVerticalAlign>,
    child_items_width: Option<&'static str>,
    control_representation: Option<&'static str>,
    collapsed: Option<bool>,
    horizontal_align: Option<&'static str>,
    vertical_align: Option<&'static str>,
    through_align: Option<&'static str>,
    united: Option<bool>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormUsualGroupGroupVerticalAlign {
    Top,
    Center,
    Bottom,
}

impl FormUsualGroupGroupVerticalAlign {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Center => "Center",
            Self::Bottom => "Bottom",
        }
    }
}

impl FormUsualGroupSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;
    const GROUP_VERTICAL_ALIGN_REVERSE_OFFSET: usize = 2;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        matches!(
            (
                wrapper,
                item_tag,
                direct_discriminator,
                options.len(),
                options.first().map(|field| field.trim()),
            ),
            ("22", "UsualGroup", Some("5"), 29, Some("29"))
        )
        .then_some(Self)
    }

    pub(crate) fn properties(self, fields: &[&str], options: &[&str]) -> FormUsualGroupProperties {
        FormUsualGroupProperties {
            enable_content_change: (fields.get(9).map(|field| field.trim()) == Some("1"))
                .then_some(true),
            group_vertical_align: self.group_vertical_align(fields),
            child_items_width: options.get(2).and_then(|field| match field.trim() {
                "1" => Some("Equal"),
                "2" => Some("LeftWide"),
                "3" => Some("LeftWidest"),
                "4" => Some("LeftNarrow"),
                "5" => Some("LeftNarrowest"),
                _ => None,
            }),
            control_representation: (options.get(11).map(|field| field.trim()) == Some("1"))
                .then_some("Picture"),
            collapsed: (options.get(12).map(|field| field.trim()) == Some("1")).then_some(true),
            horizontal_align: options.get(17).and_then(|field| match field.trim() {
                "0" => Some("Left"),
                "1" => Some("Center"),
                "2" => Some("Right"),
                _ => None,
            }),
            vertical_align: options.get(18).and_then(|field| match field.trim() {
                "0" => Some("Top"),
                "1" => Some("Center"),
                "2" => Some("Bottom"),
                _ => None,
            }),
            through_align: options.get(19).and_then(|field| match field.trim() {
                "0" => Some("Use"),
                "1" => Some("DontUse"),
                _ => None,
            }),
            united: (options.get(21).map(|field| field.trim()) == Some("0")).then_some(false),
        }
    }

    fn group_vertical_align(self, fields: &[&str]) -> Option<FormUsualGroupGroupVerticalAlign> {
        let slot = fields
            .len()
            .checked_sub(Self::GROUP_VERTICAL_ALIGN_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "0" => Some(FormUsualGroupGroupVerticalAlign::Top),
            "1" => Some(FormUsualGroupGroupVerticalAlign::Center),
            "2" => Some(FormUsualGroupGroupVerticalAlign::Bottom),
            _ => None,
        }
    }
}

impl FormUsualGroupProperties {
    pub(crate) const fn enable_content_change(self) -> Option<bool> {
        self.enable_content_change
    }

    pub(crate) const fn group_vertical_align(self) -> Option<FormUsualGroupGroupVerticalAlign> {
        self.group_vertical_align
    }

    pub(crate) const fn child_items_width(self) -> Option<&'static str> {
        self.child_items_width
    }

    pub(crate) const fn control_representation(self) -> Option<&'static str> {
        self.control_representation
    }

    pub(crate) const fn collapsed(self) -> Option<bool> {
        self.collapsed
    }

    pub(crate) const fn horizontal_align(self) -> Option<&'static str> {
        self.horizontal_align
    }

    pub(crate) const fn vertical_align(self) -> Option<&'static str> {
        self.vertical_align
    }

    pub(crate) const fn through_align(self) -> Option<&'static str> {
        self.through_align
    }

    pub(crate) const fn united(self) -> Option<bool> {
        self.united
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormPictureValueKind {
    Empty,
    Reference,
    Embedded,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPictureValueSchema {
    kind: FormPictureValueKind,
    load_transparent: bool,
}

impl FormPictureValueSchema {
    fn from_raw_layout(value: &[&str]) -> Option<Self> {
        if value.first().map(|field| field.trim()) != Some("4")
            || value.get(3).map(|field| field.trim()) != Some("\"\"")
            || value.get(4).map(|field| field.trim()) != Some("-1")
            || value.get(5).map(|field| field.trim()) != Some("-1")
        {
            return None;
        }
        let load_transparent = match value.get(6).map(|field| field.trim()) {
            Some("0") => false,
            Some("1") => true,
            _ => return None,
        };
        let kind =
            match value.get(1).map(|field| field.trim()) {
                Some("0")
                    if value.len() == 9
                        && value.get(2).map(|field| field.trim()) == Some("{0}")
                        && value.get(7).map(|field| field.trim()) == Some("0")
                        && value.get(8).map(|field| field.trim()) == Some("\"\"") =>
                {
                    FormPictureValueKind::Empty
                }
                Some("1")
                    if value.len() == 9
                        && value.get(2).map(|field| field.trim()).is_some_and(|field| {
                            field.starts_with('{') && field.ends_with('}')
                        })
                        && value.get(7).map(|field| field.trim()) == Some("0")
                        && value.get(8).map(|field| field.trim()) == Some("\"\"") =>
                {
                    FormPictureValueKind::Reference
                }
                Some("3")
                    if value.len() == 10
                        && value.get(2).map(|field| field.trim()) == Some("{0}")
                        && value.get(7).map(|field| field.trim()).is_some_and(|field| {
                            field.starts_with('{') && field.ends_with('}')
                        })
                        && value.get(8).map(|field| field.trim()) == Some("0")
                        && value.get(9).map(|field| field.trim()) == Some("\"\"") =>
                {
                    FormPictureValueKind::Embedded
                }
                _ => return None,
            };
        Some(Self {
            kind,
            load_transparent,
        })
    }

    pub(crate) const fn kind(self) -> FormPictureValueKind {
        self.kind
    }

    pub(crate) const fn load_transparent(self) -> bool {
        self.load_transparent
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormCommandCurrentRowUse {
    Use,
    DontUse,
}

impl FormCommandCurrentRowUse {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Use => "Use",
            Self::DontUse => "DontUse",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormCommandSchema<'a> {
    picture: FormPictureValueSchema,
    current_row_use: Option<FormCommandCurrentRowUse>,
    associated_table_element_id: Option<&'a str>,
}

impl<'a> FormCommandSchema<'a> {
    pub(crate) fn from_raw_layout(
        fields: &'a [&'a str],
        picture_value: &[&str],
        picture_reference: &[&str],
    ) -> Option<Self> {
        if fields.len() != 19
            || !matches!(fields.first().map(|field| field.trim()), Some("9" | "11"))
        {
            return None;
        }

        let picture = FormPictureValueSchema::from_raw_layout(picture_value)?;
        let picture_reference_is_exact = match picture.kind() {
            FormPictureValueKind::Empty => {
                matches!(picture_reference, [kind] if kind.trim() == "0")
            }
            FormPictureValueKind::Reference => match picture_reference {
                [kind, uuid] => kind.trim() == "0" && !uuid.trim().is_empty(),
                [code] => code.trim().parse::<i32>().ok().is_some_and(|code| code < 0),
                _ => false,
            },
            _ => false,
        };
        if !picture_reference_is_exact {
            return None;
        }

        let current_row_use = match fields.get(18).map(|field| field.trim()) {
            Some("0") => Some(FormCommandCurrentRowUse::Use),
            Some("1") => Some(FormCommandCurrentRowUse::DontUse),
            Some("2") => None,
            _ => return None,
        };
        let associated_table_element_id = match fields.get(14).map(|field| field.trim()) {
            Some("0") => None,
            Some(id) if id.parse::<u64>().ok().is_some_and(|id| id != 0) => Some(id),
            _ => return None,
        };

        Some(Self {
            picture,
            current_row_use,
            associated_table_element_id,
        })
    }

    pub(crate) const fn picture(self) -> FormPictureValueSchema {
        self.picture
    }

    pub(crate) const fn current_row_use(self) -> Option<FormCommandCurrentRowUse> {
        self.current_row_use
    }

    pub(crate) const fn associated_table_element_id(self) -> Option<&'a str> {
        self.associated_table_element_id
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormFieldHeaderPictureSchema {
    picture_slot: usize,
    value: FormPictureValueSchema,
}

impl FormFieldHeaderPictureSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
        value: &[&str],
    ) -> Option<Self> {
        if wrapper != "37"
            || field_count != 59 + top_level_offset
            || top_level_offset > 1
            || !matches!(
                item_tag,
                "LabelField" | "InputField" | "CheckBoxField" | "PictureField"
            )
        {
            return None;
        }
        let value = FormPictureValueSchema::from_raw_layout(value)?;
        Some(Self {
            picture_slot: 29 + top_level_offset,
            value,
        })
    }

    pub(crate) const fn picture_slot(self) -> usize {
        self.picture_slot
    }

    pub(crate) const fn kind(self) -> FormPictureValueKind {
        self.value.kind()
    }

    pub(crate) const fn load_transparent(self) -> bool {
        self.value.load_transparent()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFieldHeaderPictureXmlProperty {
    Value,
    LoadTransparent,
}

pub(crate) const FORM_FIELD_HEADER_PICTURE_XML_ORDER: &[FormFieldHeaderPictureXmlProperty] = &[
    FormFieldHeaderPictureXmlProperty::Value,
    FormFieldHeaderPictureXmlProperty::LoadTransparent,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootMobileDeviceCommandBarContentSchema {
    item_count: usize,
}

impl FormRootMobileDeviceCommandBarContentSchema {
    pub(crate) const ROOT_TRAILER_FIELDS: usize = 24;
    pub(crate) const CONTENT_TRAILER_SLOT: usize = 22;

    pub(crate) fn from_raw_layout(
        root_marker: Option<&str>,
        trailer_len: usize,
        content_kind: Option<&str>,
        content_field_count: usize,
        declared_item_count: usize,
        typed_item_count: usize,
    ) -> Option<Self> {
        let expected_field_count = declared_item_count.checked_mul(2)?.checked_add(2)?;
        (root_marker == Some("50")
            && trailer_len == Self::ROOT_TRAILER_FIELDS
            && content_kind == Some("50")
            && content_field_count == expected_field_count
            && typed_item_count == declared_item_count)
            .then_some(Self {
                item_count: declared_item_count,
            })
    }

    pub(crate) const fn item_count(self) -> usize {
        self.item_count
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormMobileDeviceCommandBarContentItemXmlProperty {
    Presentation,
    CheckState,
    Value,
}

pub(crate) const FORM_MOBILE_DEVICE_COMMAND_BAR_CONTENT_ITEM_XML_ORDER:
    &[FormMobileDeviceCommandBarContentItemXmlProperty] = &[
    FormMobileDeviceCommandBarContentItemXmlProperty::Presentation,
    FormMobileDeviceCommandBarContentItemXmlProperty::CheckState,
    FormMobileDeviceCommandBarContentItemXmlProperty::Value,
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
pub(crate) enum FormPictureDecorationGeometryXmlProperty {
    Width,
    AutoMaxWidth,
    MaxWidth,
    Height,
    AutoMaxHeight,
    MaxHeight,
    HorizontalStretch,
    VerticalStretch,
}

pub(crate) const FORM_PICTURE_DECORATION_GEOMETRY_XML_ORDER:
    &[FormPictureDecorationGeometryXmlProperty] = &[
    FormPictureDecorationGeometryXmlProperty::Width,
    FormPictureDecorationGeometryXmlProperty::AutoMaxWidth,
    FormPictureDecorationGeometryXmlProperty::MaxWidth,
    FormPictureDecorationGeometryXmlProperty::Height,
    FormPictureDecorationGeometryXmlProperty::AutoMaxHeight,
    FormPictureDecorationGeometryXmlProperty::MaxHeight,
    FormPictureDecorationGeometryXmlProperty::HorizontalStretch,
    FormPictureDecorationGeometryXmlProperty::VerticalStretch,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPictureDecorationSchema;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormPictureDecorationProperties {
    width: Option<String>,
    auto_max_width: Option<bool>,
    max_width: Option<String>,
    height: Option<String>,
    auto_max_height: Option<bool>,
    max_height: Option<String>,
    horizontal_stretch: Option<bool>,
    vertical_stretch: Option<bool>,
    skip_on_input: Option<bool>,
    group_horizontal_align: Option<&'static str>,
    group_vertical_align: Option<&'static str>,
}

impl FormPictureDecorationSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
    ) -> Option<Self> {
        matches!(
            (wrapper, field_count, item_tag, direct_discriminator),
            ("12", 36, "PictureDecoration", Some("1"))
        )
        .then_some(Self)
    }

    pub(crate) fn properties(self, fields: &[&str]) -> FormPictureDecorationProperties {
        FormPictureDecorationProperties {
            width: Self::non_zero_u32(fields, 10),
            height: Self::non_zero_u32(fields, 11),
            horizontal_stretch: Self::stretch(fields, 12),
            vertical_stretch: Self::stretch(fields, 13),
            skip_on_input: Self::bool_or_omit(fields, 22),
            auto_max_width: Self::false_or_omit(fields, 27),
            max_width: Self::non_zero_u32(fields, 28),
            auto_max_height: Self::false_or_omit(fields, 30),
            max_height: Self::non_zero_u32(fields, 31),
            group_horizontal_align: fields.get(32).and_then(|field| match field.trim() {
                "0" => Some("Left"),
                "1" => Some("Center"),
                "2" => Some("Right"),
                _ => None,
            }),
            group_vertical_align: fields.get(33).and_then(|field| match field.trim() {
                "0" => Some("Top"),
                "1" => Some("Center"),
                _ => None,
            }),
        }
    }

    fn non_zero_u32(fields: &[&str], slot: usize) -> Option<String> {
        let value = fields.get(slot)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }

    fn false_or_omit(fields: &[&str], slot: usize) -> Option<bool> {
        (fields.get(slot)?.trim() == "0").then_some(false)
    }

    fn bool_or_omit(fields: &[&str], slot: usize) -> Option<bool> {
        match fields.get(slot)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }

    fn stretch(fields: &[&str], slot: usize) -> Option<bool> {
        Self::bool_or_omit(fields, slot)
    }
}

impl FormPictureDecorationProperties {
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

    pub(crate) const fn skip_on_input(&self) -> Option<bool> {
        self.skip_on_input
    }

    pub(crate) const fn group_horizontal_align(&self) -> Option<&'static str> {
        self.group_horizontal_align
    }

    pub(crate) const fn group_vertical_align(&self) -> Option<&'static str> {
        self.group_vertical_align
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
    display_importance_slot: usize,
    horizontal_align_option_slot: usize,
    vertical_align_option_slot: usize,
    title_height_option_slot: usize,
    title_values_slot: usize,
    title_formatted_slot: usize,
}

impl FormLabelDecorationSchema {
    pub(crate) const OPTIONS_SLOT: usize = 18;
    pub(crate) const TITLE_SLOT: usize = 23;

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
                display_importance_slot: 34,
                horizontal_align_option_slot: 2,
                vertical_align_option_slot: 3,
                title_height_option_slot: 4,
                title_values_slot: 1,
                title_formatted_slot: 2,
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

    pub(crate) fn display_importance(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(self.display_importance_slot)?.trim() {
            "1" => Some("VeryHigh"),
            "5" => Some("VeryLow"),
            _ => None,
        }
    }

    pub(crate) const fn horizontal_align_option_slot(self) -> usize {
        self.horizontal_align_option_slot
    }

    pub(crate) const fn vertical_align_option_slot(self) -> usize {
        self.vertical_align_option_slot
    }

    pub(crate) fn title_schema(self, title: &[&str]) -> Option<FormLabelDecorationTitleSchema> {
        matches!(
            (
                title.len(),
                title.first().map(|field| field.trim()),
                title
                    .get(self.title_formatted_slot)
                    .map(|field| field.trim()),
            ),
            (3, Some("1"), Some("0" | "1"))
        )
        .then_some(FormLabelDecorationTitleSchema {
            values_slot: self.title_values_slot,
            formatted_slot: self.title_formatted_slot,
        })
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormLabelDecorationTitleSchema {
    values_slot: usize,
    formatted_slot: usize,
}

impl FormLabelDecorationTitleSchema {
    pub(crate) const fn values_slot(self) -> usize {
        self.values_slot
    }

    pub(crate) const fn formatted_slot(self) -> usize {
        self.formatted_slot
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormFieldSchema {
    top_level_offset: usize,
    title_slot: usize,
    text_color_option_slot: Option<usize>,
    back_color_option_slot: Option<usize>,
    border_color_option_slot: Option<usize>,
    extended_edit_multiple_values_option_slot: Option<usize>,
}

impl FormFieldSchema {
    pub(crate) const OPTIONS_BASE_SLOT: usize = 39;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        let (discriminator, options_len, options_kind, text, back, border) = match item_tag {
            "LabelField" => ("1", 20, "11", Some(8), Some(9), None),
            "InputField" => ("2", 66, "36", Some(37), Some(38), Some(39)),
            "CheckBoxField" => ("3", 13, "11", None, None, None),
            "PictureField" => ("4", 24, "10", None, None, None),
            "RadioButtonField" => ("5", 12, "8", None, None, None),
            "SpreadSheetDocumentField" => ("6", 32, "13", None, None, Some(15)),
            "TextDocumentField" => ("7", 16, "5", None, None, None),
            "CalendarField" => ("8", 24, "6", None, None, None),
            "GraphicalSchemaField" => ("14", 14, "3", None, None, None),
            "HTMLDocumentField" => ("15", 13, "3", None, None, Some(3)),
            "FormattedDocumentField" => ("17", 16, "1", None, None, None),
            _ => return None,
        };
        if wrapper != "37"
            || field_count != 59 + top_level_offset
            || top_level_offset > 1
            || (top_level_offset == 1
                && !matches!(
                    item_tag,
                    "LabelField" | "InputField" | "CheckBoxField" | "PictureField"
                ))
            || direct_discriminator != Some(discriminator)
            || options.len() != options_len
            || options.first().map(|field| field.trim()) != Some(options_kind)
        {
            return None;
        }
        Some(Self {
            top_level_offset,
            title_slot: 9 + top_level_offset,
            text_color_option_slot: text,
            back_color_option_slot: back,
            border_color_option_slot: border,
            extended_edit_multiple_values_option_slot: (item_tag == "InputField")
                .then_some(FormInputFieldExtendedOptionSlot::ExtendedEditMultipleValues.index()),
        })
    }

    pub(crate) const fn title_slot(self) -> usize {
        self.title_slot
    }

    pub(crate) fn footer_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        (fields.get(25 + self.top_level_offset)?.trim() == "0").then_some("Left")
    }

    pub(crate) fn skip_on_input(self, fields: &[&str]) -> Option<bool> {
        match fields.get(15 + self.top_level_offset)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }

    pub(crate) fn picture_field_file_drag_mode(self, options: &[&str]) -> Option<&'static str> {
        (options.get(22)?.trim() == "0").then_some("AsFile")
    }

    pub(crate) fn extended_edit_multiple_values(self, options: &[&str]) -> Option<bool> {
        match options
            .get(self.extended_edit_multiple_values_option_slot?)?
            .trim()
        {
            "1" => Some(true),
            "0" => None,
            _ => None,
        }
    }

    pub(crate) const fn text_color_option_slot(self) -> Option<usize> {
        self.text_color_option_slot
    }

    pub(crate) const fn back_color_option_slot(self) -> Option<usize> {
        self.back_color_option_slot
    }

    pub(crate) const fn border_color_option_slot(self) -> Option<usize> {
        self.border_color_option_slot
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormButtonColorSchema;

impl FormButtonColorSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
    ) -> Option<Self> {
        (wrapper == "31" && field_count == 52 && item_tag == "Button").then_some(Self)
    }

    pub(crate) const fn back_color_slot(self) -> usize {
        19
    }

    pub(crate) const fn text_color_slot(self) -> usize {
        20
    }

    pub(crate) const fn border_color_slot(self) -> usize {
        21
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormButtonShapeRepresentationSchema {
    slot: usize,
}

impl FormButtonShapeRepresentationSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
    ) -> Option<Self> {
        match (wrapper, field_count, item_tag, top_level_offset) {
            ("31", 52, "Button", 0) => Some(Self { slot: 45 }),
            _ => None,
        }
    }

    pub(crate) fn shape_representation(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(self.slot)?.trim() {
            "1" => Some("Always"),
            "2" => Some("WhenActive"),
            "3" => Some("None"),
            _ => None,
        }
    }
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
pub(crate) struct FormChildItemUserVisibleSchema;

impl FormChildItemUserVisibleSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
        conditional_marker: Option<&str>,
        user_visible_common: Option<bool>,
    ) -> Option<Self> {
        match (
            wrapper,
            field_count,
            item_tag,
            top_level_offset,
            conditional_marker,
            user_visible_common,
        ) {
            ("31", 53, "Button", 1, Some("1"), Some(false))
            | ("37", 60, "PictureField", 1, Some("1"), Some(false)) => Some(Self),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormChildItemVisibleSchema {
    slot: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormConditionalGroupSchema {
    prefix_slot: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormConditionalTableSchema {
    prefix_slot: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormCommandInterfaceContainerOwner {
    CommandBar,
    NavigationPanel,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormCommandInterfaceContainerSchema {
    owner: FormCommandInterfaceContainerOwner,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormCommandInterfaceItemSchema;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormCommandInterfaceVisibilitySchema {
    role_count: usize,
}

impl FormCommandInterfaceContainerSchema {
    pub(crate) fn from_raw_layout(
        trailing_slot: usize,
        wrapper: &str,
        field_count: usize,
        declared_item_count: usize,
        typed_item_count: usize,
    ) -> Option<Self> {
        let owner = match trailing_slot {
            3 => FormCommandInterfaceContainerOwner::NavigationPanel,
            4 => FormCommandInterfaceContainerOwner::CommandBar,
            _ => return None,
        };
        (wrapper == "0"
            && declared_item_count > 0
            && field_count == declared_item_count.checked_add(2)?
            && typed_item_count == declared_item_count)
            .then_some(Self { owner })
    }

    pub(crate) const fn owner(self) -> FormCommandInterfaceContainerOwner {
        self.owner
    }
}

impl FormCommandInterfaceItemSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_type: &str,
        default_visible: &str,
    ) -> Option<Self> {
        (wrapper == "3"
            && field_count == 9
            && matches!(item_type, "0" | "1")
            && matches!(default_visible, "0" | "1"))
        .then_some(Self)
    }
}

impl FormCommandInterfaceVisibilitySchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        scope_wrapper: &str,
        scope_field_count: usize,
        role_count: usize,
        typed_role_count: usize,
    ) -> Option<Self> {
        let expected_scope_fields = role_count.checked_mul(2)?.checked_add(3)?;
        (wrapper == "0"
            && field_count == 2
            && scope_wrapper == "0"
            && scope_field_count == expected_scope_fields
            && typed_role_count == role_count)
            .then_some(Self { role_count })
    }

    pub(crate) const fn role_count(self) -> usize {
        self.role_count
    }
}

impl FormConditionalGroupSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        user_visible_common: Option<bool>,
        shifted_discriminator: Option<&str>,
    ) -> Option<Self> {
        match (
            wrapper,
            field_count,
            user_visible_common,
            shifted_discriminator,
        ) {
            ("22", field_count, Some(false), Some("2" | "3" | "5"))
                if field_count >= 31 && (field_count - 31) % 2 == 0 =>
            {
                Some(Self { prefix_slot: 5 })
            }
            _ => None,
        }
    }

    pub(crate) const fn prefix_slot(self) -> usize {
        self.prefix_slot
    }
}

impl FormConditionalTableSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        user_visible_common: Option<bool>,
        conditional_marker: Option<&str>,
    ) -> Option<Self> {
        match (
            wrapper,
            field_count,
            user_visible_common,
            conditional_marker,
        ) {
            ("55", field_count, Some(false), Some("1"))
                if field_count >= 100 && (field_count - 100) % 2 == 0 =>
            {
                Some(Self { prefix_slot: 5 })
            }
            _ => None,
        }
    }

    pub(crate) const fn prefix_slot(self) -> usize {
        self.prefix_slot
    }
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
            // Preserve the three wrapper-48 field owners decoded by the legacy path.
            ("48", "LabelField", Some("1"))
            | ("48", "InputField", Some("2"))
            | ("48", "CheckBoxField", Some("3"))
                if field_count > 20 =>
            {
                43 + top_level_offset
            }
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
pub(crate) struct FormChildItemShowTitleSchema {
    option_slot: usize,
    back_color_option_slot: Option<usize>,
}

impl FormChildItemShowTitleSchema {
    pub(crate) const OPTIONS_SLOT: usize = FormPageSchema::OPTIONS_SLOT;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        if item_tag == "Page" {
            FormPageSchema::from_raw_layout(
                wrapper,
                field_count,
                item_tag,
                direct_discriminator,
                options,
            )?;
            return Some(Self {
                option_slot: 6,
                back_color_option_slot: Some(9),
            });
        }
        if wrapper != "22" || field_count < 30 || (field_count - 30) % 2 != 0 {
            return None;
        }
        let (option_slot, back_color_option_slot) = match (
            item_tag,
            direct_discriminator,
            options.len(),
            options.first().map(|field| field.trim()),
        ) {
            ("ColumnGroup", Some("2"), 12, Some("2")) => (2, None),
            ("UsualGroup", Some("5"), 29, Some("29")) => (4, Some(9)),
            _ => return None,
        };
        Some(Self {
            option_slot,
            back_color_option_slot,
        })
    }

    pub(crate) fn show_title(self, options: &[&str]) -> Option<bool> {
        (options.get(self.option_slot)?.trim() == "0").then_some(false)
    }

    pub(crate) const fn back_color_option_slot(self) -> Option<usize> {
        self.back_color_option_slot
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootVerticalScrollSchema {
    qualifier_slot: usize,
    mode_slot: usize,
}

impl FormRootVerticalScrollSchema {
    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer_field_count: usize,
    ) -> Option<Self> {
        matches!((root_discriminator, trailer_field_count), (Some("50"), 24)).then_some(Self {
            qualifier_slot: 5,
            mode_slot: 15,
        })
    }

    pub(crate) fn vertical_scroll(self, trailer: &[&str]) -> Option<&'static str> {
        match (
            trailer.get(self.qualifier_slot).map(|field| field.trim()),
            trailer.get(self.mode_slot).map(|field| field.trim()),
        ) {
            (Some("2"), Some("2")) => Some("useIfNecessary"),
            (Some("0"), Some("3")) => Some("useWithoutStretch"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootAutoUrlSchema {
    auto_url: Option<bool>,
}

impl FormRootAutoUrlSchema {
    const AUTO_URL_SLOT: usize = 3;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer: &[&str],
    ) -> Option<Self> {
        if root_discriminator != Some("50") || trailer.len() != 24 {
            return None;
        }
        let auto_url = match trailer.get(Self::AUTO_URL_SLOT)?.trim() {
            "0" => Some(false),
            "1" => None,
            _ => return None,
        };
        Some(Self { auto_url })
    }

    pub(crate) fn from_legacy_raw_layout(
        root_discriminator: Option<&str>,
        fields: &[&str],
        uses_property_bag: bool,
    ) -> Option<Self> {
        if root_discriminator != Some("59") || uses_property_bag {
            return None;
        }
        let auto_url = match (
            fields.get(11).map(|field| field.trim()),
            fields.get(13).map(|field| field.trim()),
        ) {
            (Some("0"), Some("0")) => Some(false),
            (Some("0"), Some("1")) => None,
            _ => return None,
        };
        Some(Self { auto_url })
    }

    pub(crate) const fn auto_url(self) -> Option<bool> {
        self.auto_url
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootGroupSchema {
    group: Option<&'static str>,
}

impl FormRootGroupSchema {
    const GROUP_KIND_SLOT: usize = 14;
    const GROUP_VALUE_SLOT: usize = 21;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        header_group_marker: Option<&str>,
        trailer: &[&str],
    ) -> Option<Self> {
        if root_discriminator != Some("50") || trailer.len() != 24 {
            return None;
        }
        let group = match (
            header_group_marker.map(str::trim),
            trailer.get(Self::GROUP_KIND_SLOT).map(|field| field.trim()),
            trailer
                .get(Self::GROUP_VALUE_SLOT)
                .map(|field| field.trim()),
        ) {
            (Some("0"), Some("0"), Some("0")) => None,
            (Some("1"), Some("1"), Some("1")) => Some("Horizontal"),
            (Some("1"), Some("2"), Some("2")) => Some("HorizontalIfPossible"),
            (Some("1"), Some("1"), Some("3")) => Some("AlwaysHorizontal"),
            _ => return None,
        };
        Some(Self { group })
    }

    pub(crate) fn from_legacy_raw_layout(
        root_discriminator: Option<&str>,
        fields: &[&str],
    ) -> Option<Self> {
        matches!(
            (
                root_discriminator,
                fields.get(11).map(|field| field.trim()),
                fields.get(13).map(|field| field.trim()),
                fields.get(14).map(|field| field.trim()),
            ),
            (Some("59"), Some("1"), Some("0"), Some("0"))
        )
        .then_some(Self {
            group: Some("Horizontal"),
        })
    }

    pub(crate) const fn group(self) -> Option<&'static str> {
        self.group
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormSpecialFieldKind {
    ProgressBar,
    TrackBar,
    Chart,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormSpecialFieldSchema {
    kind: FormSpecialFieldKind,
}

impl FormSpecialFieldSchema {
    pub(crate) const OPTIONS_SLOT: usize = 39;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        discriminator: Option<&str>,
        option_count: usize,
        option_kind: Option<&str>,
    ) -> Option<Self> {
        let kind = match (
            wrapper,
            field_count,
            discriminator,
            option_count,
            option_kind,
        ) {
            ("37", 59, Some("9"), 16, Some("4")) => FormSpecialFieldKind::ProgressBar,
            ("37", 59, Some("10"), 18, Some("2")) => FormSpecialFieldKind::TrackBar,
            ("37", 59, Some("11"), 11, Some("1")) => FormSpecialFieldKind::Chart,
            _ => return None,
        };
        Some(Self { kind })
    }

    pub(crate) const fn xml_tag(self) -> &'static str {
        match self.kind {
            FormSpecialFieldKind::ProgressBar => "ProgressBarField",
            FormSpecialFieldKind::TrackBar => "TrackBarField",
            FormSpecialFieldKind::Chart => "ChartField",
        }
    }

    pub(crate) fn width(self, options: &[&str]) -> Option<String> {
        let value = options.get(1)?.trim();
        let is_non_default = match self.kind {
            FormSpecialFieldKind::ProgressBar => value != "0" && value != "32",
            FormSpecialFieldKind::TrackBar => value != "0",
            FormSpecialFieldKind::Chart => false,
        };
        (is_non_default && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }

    pub(crate) fn auto_max_width(self, options: &[&str]) -> Option<bool> {
        match self.kind {
            FormSpecialFieldKind::ProgressBar
                if options.get(11).map(|field| field.trim()) == Some("0") =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    pub(crate) fn horizontal_stretch(self, options: &[&str]) -> Option<bool> {
        match self.kind {
            FormSpecialFieldKind::TrackBar
                if options.get(3).map(|field| field.trim()) == Some("0") =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        match (self.kind, fields.get(54).map(|field| field.trim())) {
            (FormSpecialFieldKind::ProgressBar, Some("1")) => Some("Center"),
            _ => None,
        }
    }

    pub(crate) fn max_value(self, options: &[&str]) -> Option<String> {
        if self.kind != FormSpecialFieldKind::ProgressBar {
            return None;
        }
        let value = options.get(6)?.trim();
        (value != "100" && value.parse::<i64>().is_ok()).then(|| value.to_string())
    }

    pub(crate) fn show_percent(self, options: &[&str]) -> Option<bool> {
        matches!(
            (self.kind, options.get(9).map(|field| field.trim())),
            (FormSpecialFieldKind::ProgressBar, Some("1"))
        )
        .then_some(true)
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
    ProgressBarField,
    TrackBarField,
    ChartField,
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
            "ProgressBarField" => Self::ProgressBarField,
            "TrackBarField" => Self::TrackBarField,
            "ChartField" => Self::ChartField,
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
        | ("37", 59, FormTooltipRepresentationItemKind::CalendarField, Some("8"))
        | ("37", 59, FormTooltipRepresentationItemKind::ProgressBarField, Some("9"))
        | ("37", 59, FormTooltipRepresentationItemKind::TrackBarField, Some("10"))
        | ("37", 59, FormTooltipRepresentationItemKind::ChartField, Some("11")) => 50,
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
        | FormTooltipRepresentationItemKind::CalendarField
        | FormTooltipRepresentationItemKind::ProgressBarField
        | FormTooltipRepresentationItemKind::TrackBarField
        | FormTooltipRepresentationItemKind::ChartField => {
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
pub(crate) enum FormInputFieldTailXmlProperty {
    ListChoiceMode,
    ExtendedEditMultipleValues,
    AutoMarkIncomplete,
}

pub(crate) const FORM_INPUT_FIELD_TAIL_XML_ORDER: &[FormInputFieldTailXmlProperty] = &[
    FormInputFieldTailXmlProperty::ListChoiceMode,
    FormInputFieldTailXmlProperty::ExtendedEditMultipleValues,
    FormInputFieldTailXmlProperty::AutoMarkIncomplete,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableXmlProperty {
    Representation,
    UserVisible,
    Visible,
    CommandBarLocation,
    Autofill,
    ReadOnly,
    SkipOnInput,
    DefaultItem,
    ChangeRowSet,
    ChangeRowOrder,
    Width,
    AutoMaxWidth,
    Height,
    AutoMaxHeight,
    HeightInTableRows,
    ChoiceMode,
    RowInputMode,
    SelectionMode,
    RowSelectionMode,
    Header,
    HorizontalLines,
    VerticalLines,
    UseAlternationRowColor,
    AutoInsertNewRow,
    AutoMarkIncomplete,
    InitialTreeView,
    EnableStartDrag,
    EnableDrag,
    FileDragMode,
    DataPath,
    RowPictureDataPath,
    RowsPicture,
    BackColor,
    TextColor,
    BorderColor,
    Title,
    CommandSet,
    SearchStringLocation,
    ViewStatusLocation,
    SearchControlLocation,
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
    FormTableXmlProperty::UserVisible,
    FormTableXmlProperty::Visible,
    FormTableXmlProperty::CommandBarLocation,
    FormTableXmlProperty::Autofill,
    FormTableXmlProperty::ReadOnly,
    FormTableXmlProperty::SkipOnInput,
    FormTableXmlProperty::DefaultItem,
    FormTableXmlProperty::ChangeRowSet,
    FormTableXmlProperty::ChangeRowOrder,
    FormTableXmlProperty::Width,
    FormTableXmlProperty::AutoMaxWidth,
    FormTableXmlProperty::Height,
    FormTableXmlProperty::AutoMaxHeight,
    FormTableXmlProperty::HeightInTableRows,
    FormTableXmlProperty::ChoiceMode,
    FormTableXmlProperty::RowInputMode,
    FormTableXmlProperty::SelectionMode,
    FormTableXmlProperty::RowSelectionMode,
    FormTableXmlProperty::Header,
    FormTableXmlProperty::HorizontalLines,
    FormTableXmlProperty::VerticalLines,
    FormTableXmlProperty::UseAlternationRowColor,
    FormTableXmlProperty::AutoInsertNewRow,
    FormTableXmlProperty::AutoMarkIncomplete,
    FormTableXmlProperty::InitialTreeView,
    FormTableXmlProperty::EnableStartDrag,
    FormTableXmlProperty::EnableDrag,
    FormTableXmlProperty::FileDragMode,
    FormTableXmlProperty::DataPath,
    FormTableXmlProperty::RowPictureDataPath,
    FormTableXmlProperty::RowsPicture,
    FormTableXmlProperty::BackColor,
    FormTableXmlProperty::TextColor,
    FormTableXmlProperty::BorderColor,
    FormTableXmlProperty::Title,
    FormTableXmlProperty::CommandSet,
    FormTableXmlProperty::SearchStringLocation,
    FormTableXmlProperty::ViewStatusLocation,
    FormTableXmlProperty::SearchControlLocation,
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
enum FormTableSlot {
    Autofill,
    ReadOnly,
    DefaultItem,
    ChangeRowSet,
    ChangeRowOrder,
    Width,
    Height,
    ChoiceMode,
    RowInputMode,
    SelectionMode,
    RowSelectionMode,
    Header,
    HorizontalLines,
    VerticalLines,
    UseAlternationRowColor,
    AutoInsertNewRow,
    EnableStartDrag,
    EnableDrag,
}

impl FormTableSlot {
    const ALL: [Self; 18] = [
        Self::Autofill,
        Self::ReadOnly,
        Self::DefaultItem,
        Self::ChangeRowSet,
        Self::ChangeRowOrder,
        Self::Width,
        Self::Height,
        Self::ChoiceMode,
        Self::RowInputMode,
        Self::SelectionMode,
        Self::RowSelectionMode,
        Self::Header,
        Self::HorizontalLines,
        Self::VerticalLines,
        Self::UseAlternationRowColor,
        Self::AutoInsertNewRow,
        Self::EnableStartDrag,
        Self::EnableDrag,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Autofill => 12,
            Self::ReadOnly => 14,
            Self::DefaultItem => 16,
            Self::ChangeRowSet => 17,
            Self::ChangeRowOrder => 18,
            Self::Width => 19,
            Self::Height => 20,
            Self::ChoiceMode => 22,
            Self::RowInputMode => 23,
            Self::SelectionMode => 24,
            Self::RowSelectionMode => 25,
            Self::Header => 26,
            Self::HorizontalLines => 32,
            Self::VerticalLines => 33,
            Self::UseAlternationRowColor => 36,
            Self::AutoInsertNewRow => 37,
            Self::EnableStartDrag => 52,
            Self::EnableDrag => 53,
        }
    }

    fn accepts(self, field: &str) -> bool {
        match self {
            Self::RowInputMode => matches!(field.trim(), "0" | "2"),
            Self::Width | Self::Height => field.trim().parse::<u32>().is_ok(),
            _ => matches!(field.trim(), "0" | "1"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableRowPictureDataPath<'a> {
    Empty,
    Payload(&'a str),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableSearchStringLocation {
    None,
    CommandBar,
    Top,
}

impl FormTableSearchStringLocation {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::CommandBar => "CommandBar",
            Self::Top => "Top",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableViewStatusLocation {
    None,
    Top,
}

impl FormTableViewStatusLocation {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Top => "Top",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableSearchControlLocation {
    None,
    CommandBar,
}

impl FormTableSearchControlLocation {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::CommandBar => "CommandBar",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormTableSchema;

impl FormTableSchema {
    const BASE_FIELD_COUNT: usize = 99;
    const COMMAND_SET_PAIR_COUNT_SLOT: usize = 54;
    const DATA_PATH_SLOT: usize = 11;
    const ROW_PICTURE_DATA_PATH_SLOT: usize = 43;
    const ROWS_PICTURE_SLOT: usize = 44;
    const BACK_COLOR_SLOT: usize = 45;
    const TEXT_COLOR_SLOT: usize = 46;
    const BORDER_COLOR_SLOT: usize = 47;
    const SEARCH_STRING_LOCATION_REVERSE_OFFSET: usize = 25;
    const VIEW_STATUS_LOCATION_REVERSE_OFFSET: usize = 24;
    const SEARCH_CONTROL_LOCATION_REVERSE_OFFSET: usize = 23;

    pub(crate) fn from_raw_layout(wrapper: &str, item_tag: &str, fields: &[&str]) -> Option<Self> {
        if wrapper != "55"
            || item_tag != "Table"
            || fields.first().map(|field| field.trim()) != Some("55")
            || fields.len() < Self::BASE_FIELD_COUNT
            || (fields.len() - Self::BASE_FIELD_COUNT) % 2 != 0
            || !FormTableSlot::ALL.iter().all(|slot| {
                fields
                    .get(slot.index())
                    .is_some_and(|field| slot.accepts(field))
            })
        {
            return None;
        }
        Some(Self)
    }

    pub(crate) const fn command_set_pair_count_slot(self) -> usize {
        Self::COMMAND_SET_PAIR_COUNT_SLOT
    }

    pub(crate) const fn data_path_slot(self) -> usize {
        Self::DATA_PATH_SLOT
    }

    pub(crate) const fn row_picture_data_path_slot(self) -> usize {
        Self::ROW_PICTURE_DATA_PATH_SLOT
    }

    pub(crate) fn row_picture_data_path<'a>(
        self,
        value: &[&'a str],
    ) -> Option<FormTableRowPictureDataPath<'a>> {
        match value {
            [marker] if marker.trim() == "0" => Some(FormTableRowPictureDataPath::Empty),
            [marker, payload] if marker.trim() == "1" => {
                Some(FormTableRowPictureDataPath::Payload(payload.trim()))
            }
            _ => None,
        }
    }

    pub(crate) const fn rows_picture_slot(self) -> usize {
        Self::ROWS_PICTURE_SLOT
    }

    pub(crate) const fn back_color_slot(self) -> usize {
        Self::BACK_COLOR_SLOT
    }

    pub(crate) const fn text_color_slot(self) -> usize {
        Self::TEXT_COLOR_SLOT
    }

    pub(crate) const fn border_color_slot(self) -> usize {
        Self::BORDER_COLOR_SLOT
    }

    pub(crate) fn search_string_location(
        self,
        fields: &[&str],
    ) -> Option<FormTableSearchStringLocation> {
        let slot = fields
            .len()
            .checked_sub(Self::SEARCH_STRING_LOCATION_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "1" => Some(FormTableSearchStringLocation::None),
            "2" => Some(FormTableSearchStringLocation::CommandBar),
            "3" => Some(FormTableSearchStringLocation::Top),
            _ => None,
        }
    }

    pub(crate) fn view_status_location(
        self,
        fields: &[&str],
    ) -> Option<FormTableViewStatusLocation> {
        let slot = fields
            .len()
            .checked_sub(Self::VIEW_STATUS_LOCATION_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "1" => Some(FormTableViewStatusLocation::None),
            "2" => Some(FormTableViewStatusLocation::Top),
            _ => None,
        }
    }

    pub(crate) fn search_control_location(
        self,
        fields: &[&str],
    ) -> Option<FormTableSearchControlLocation> {
        let slot = fields
            .len()
            .checked_sub(Self::SEARCH_CONTROL_LOCATION_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "1" => Some(FormTableSearchControlLocation::None),
            "2" => Some(FormTableSearchControlLocation::CommandBar),
            _ => None,
        }
    }

    pub(crate) fn rows_picture(self, value: &[&str]) -> Option<FormPictureValueSchema> {
        FormPictureValueSchema::from_raw_layout(value)
    }

    pub(crate) fn autofill(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::Autofill)
    }

    pub(crate) fn read_only(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::ReadOnly)
    }

    pub(crate) fn default_item(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::DefaultItem)
    }

    pub(crate) fn change_row_set(self, fields: &[&str]) -> Option<bool> {
        self.explicit_false(fields, FormTableSlot::ChangeRowSet)
    }

    pub(crate) fn change_row_order(self, fields: &[&str]) -> Option<bool> {
        self.explicit_false(fields, FormTableSlot::ChangeRowOrder)
    }

    pub(crate) fn width(self, fields: &[&str]) -> Option<String> {
        self.non_zero_u32(fields, FormTableSlot::Width)
    }

    pub(crate) fn height(self, fields: &[&str]) -> Option<String> {
        self.non_zero_u32(fields, FormTableSlot::Height)
    }

    pub(crate) fn choice_mode(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::ChoiceMode)
    }

    pub(crate) fn row_input_mode(self, fields: &[&str]) -> Option<&'static str> {
        (fields.get(FormTableSlot::RowInputMode.index())?.trim() == "2")
            .then_some("AfterCurrentRow")
    }

    pub(crate) fn selection_mode(self, fields: &[&str]) -> Option<&'static str> {
        (fields.get(FormTableSlot::SelectionMode.index())?.trim() == "0").then_some("SingleRow")
    }

    pub(crate) fn row_selection_mode(self, fields: &[&str]) -> Option<&'static str> {
        (fields.get(FormTableSlot::RowSelectionMode.index())?.trim() == "1").then_some("Row")
    }

    pub(crate) fn header(self, fields: &[&str]) -> Option<bool> {
        self.explicit_false(fields, FormTableSlot::Header)
    }

    pub(crate) fn horizontal_lines(self, fields: &[&str]) -> Option<bool> {
        self.explicit_false(fields, FormTableSlot::HorizontalLines)
    }

    pub(crate) fn vertical_lines(self, fields: &[&str]) -> Option<bool> {
        self.explicit_false(fields, FormTableSlot::VerticalLines)
    }

    pub(crate) fn use_alternation_row_color(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::UseAlternationRowColor)
    }

    pub(crate) fn auto_insert_new_row(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::AutoInsertNewRow)
    }

    pub(crate) fn enable_start_drag(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::EnableStartDrag)
    }

    pub(crate) fn enable_drag(self, fields: &[&str]) -> Option<bool> {
        self.explicit_true(fields, FormTableSlot::EnableDrag)
    }

    fn explicit_true(self, fields: &[&str], slot: FormTableSlot) -> Option<bool> {
        (fields.get(slot.index())?.trim() == "1").then_some(true)
    }

    fn explicit_false(self, fields: &[&str], slot: FormTableSlot) -> Option<bool> {
        (fields.get(slot.index())?.trim() == "0").then_some(false)
    }

    fn non_zero_u32(self, fields: &[&str], slot: FormTableSlot) -> Option<String> {
        let value = fields.get(slot.index())?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }
}

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
    ExtendedEdit,
    ChoiceListButton,
    ChoiceButton,
    ClearButton,
    SpinButton,
    OpenButton,
    ListChoiceMode,
    QuickChoice,
    AutoCellHeight,
    ChoiceFoldersAndItems,
    ChoiceParameterLinks,
    AutoChoiceIncomplete,
    AutoMarkIncomplete,
    ChooseType,
    Format,
    EditFormat,
    Font,
    TextEdit,
    TypeLink,
    CreateButton,
    ChoiceButtonRepresentation,
    DropListButton,
    AutoMaxWidth,
    MaxWidth,
    AutoMaxHeight,
    MaxHeight,
    ExtendedEditMultipleValues,
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
            Self::ExtendedEdit => 9,
            Self::ChoiceListButton => 11,
            Self::ChoiceButton => 12,
            Self::ClearButton => 13,
            Self::SpinButton => 14,
            Self::OpenButton => 15,
            Self::ListChoiceMode => 19,
            Self::QuickChoice => 23,
            Self::ChoiceFoldersAndItems => 24,
            Self::ChoiceParameterLinks => 26,
            Self::AutoCellHeight => 28,
            Self::AutoChoiceIncomplete => 28,
            Self::Format => 29,
            Self::EditFormat => 30,
            Self::AutoMarkIncomplete => 31,
            Self::ChooseType => 32,
            Self::Font => 40,
            Self::TextEdit => 41,
            Self::TypeLink => 42,
            Self::CreateButton => 45,
            Self::ChoiceButtonRepresentation => 46,
            Self::DropListButton => 47,
            Self::AutoMaxWidth => 49,
            Self::MaxWidth => 50,
            Self::AutoMaxHeight => 52,
            Self::MaxHeight => 53,
            Self::ExtendedEditMultipleValues => 65,
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
