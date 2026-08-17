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

/// Schema bridge for the sole direct child admitted by a TextDocumentField.
pub(crate) fn form_text_document_context_menu_child_is_valid(tag: &str) -> bool {
    tag == "ContextMenu"
}

// Platform type ID used by serialized Form column patterns for a DCS filter.
const FORM_DATA_COMPOSITION_FILTER_TYPE_UUID: &str = "f6841c6b-6c71-4c82-ae9e-d08b49db326c";

pub(crate) fn form_attribute_column_builtin_type_reference(type_id: &str) -> Option<&'static str> {
    type_id
        .eq_ignore_ascii_case(FORM_DATA_COMPOSITION_FILTER_TYPE_UUID)
        .then_some("dcsset:Filter")
}

/// Slot holding `FormButtonType` in the long extended Button layout, before the
/// top-level name offset is applied.
const FORM_LONG_BUTTON_TYPE_SLOT: usize = 46;
/// Slot the shorter extended Button layout uses for the same property.
const FORM_SHORT_BUTTON_TYPE_SLOT: usize = 4;
/// Field count of the long extended Button layout, before the offset.
const FORM_LONG_BUTTON_FIELD_COUNT: usize = 52;

/// Which slot of an extended Button layout carries `FormButtonType`.
///
/// Every Button the platform wrote into the UT 11.5.27.75 native tree uses the
/// long layout: 27 771 items with 52 fields and no name offset, 20 items with
/// 53 fields and offset 1 - one shape, no third variant. There, slot
/// `FORM_LONG_BUTTON_TYPE_SLOT` is the four-valued `FormButtonType` code, while
/// slot `FORM_SHORT_BUTTON_TYPE_SLOT` folds `CommandBarHyperlink` onto
/// `CommandBarButton` and so cannot express the enumeration. The short extended
/// layout is unobserved in every corpus available here (UT, the three reference
/// trees and the nine bundled configurations contain no such Button), so it
/// keeps reading the slot it always read rather than being changed on a guess.
pub(crate) fn form_extended_button_type_slot(field_count: usize, offset: usize) -> usize {
    if field_count >= FORM_LONG_BUTTON_FIELD_COUNT + offset {
        FORM_LONG_BUTTON_TYPE_SLOT + offset
    } else {
        FORM_SHORT_BUTTON_TYPE_SLOT + offset
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormAttributeAdditionalColumnsBindingKind {
    Attribute,
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
        let expected_field_count = column_count.checked_add(3)?;
        let target_arity = target.first()?.trim().parse::<usize>().ok()?;
        let expected_target_len = target_arity.checked_add(1)?;
        if fields.len() != expected_field_count
            || fields.first().map(|field| field.trim()) != Some("0")
            || target_arity < 1
            || target.len() != expected_target_len
            || owner.len() != 1
            || owner.first()?.trim().is_empty()
        {
            return None;
        }
        let binding_kind = if target_arity == 1 {
            if column_count != 0 || !binding.is_empty() {
                return None;
            }
            FormAttributeAdditionalColumnsBindingKind::Attribute
        } else {
            match binding {
                [number] if number.trim().parse::<u64>().is_ok() => {
                    FormAttributeAdditionalColumnsBindingKind::Numeric
                }
                [prefix, uuid] if prefix.trim() == "0" && !uuid.trim().is_empty() => {
                    FormAttributeAdditionalColumnsBindingKind::MetadataReference
                }
                _ => return None,
            }
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

    /// A `Popup` keeps its background colour in its own option tuple, in the
    /// slot right behind the three reserved members the layout guard pins.
    /// On all 3 809 popups of the native UT 11.5.27.75 form dumps the slot
    /// holds the unset colour on every popup without a `<BackColor>` and a
    /// readable colour on all five that carry one.
    pub(crate) const BACK_COLOR_OPTION_SLOT: usize = 7;

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
pub(crate) struct FormNestedAutoCommandBarSchema {
    marker: FormAutoCommandBarMarker,
    empty_shape: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct FormAutoCommandBarMarker {
    horizontal_align: FormAutoCommandBarHorizontalAlign,
    autofill: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormAutoCommandBarHorizontalAlign {
    Default,
    Center,
    Right,
    Auto,
}

impl FormAutoCommandBarHorizontalAlign {
    const fn xml_value(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::Center => Some("Center"),
            Self::Right => Some("Right"),
            Self::Auto => Some("Auto"),
        }
    }
}

impl FormNestedAutoCommandBarSchema {
    pub(crate) const MARKER_SLOT: usize = 20;
    const MIN_FIELD_COUNT: usize = 29;
    const MAX_MARKER_BYTES: usize = 128;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        item_tag: &str,
        item_id: &str,
        direct_discriminator: Option<&str>,
        fields: &[&str],
    ) -> Option<Self> {
        if wrapper != "22"
            || fields.len() < Self::MIN_FIELD_COUNT
            || !(fields.len() - Self::MIN_FIELD_COUNT).is_multiple_of(2)
            || item_tag != "AutoCommandBar"
            || item_id == "-1"
            || direct_discriminator != Some("9")
        {
            return None;
        }
        let marker = parse_nested_auto_command_bar_marker(fields.get(Self::MARKER_SLOT)?)?;
        let empty_shape = fields.len() == Self::MIN_FIELD_COUNT
            && marker.horizontal_align == FormAutoCommandBarHorizontalAlign::Default
            && marker.autofill;
        Some(Self {
            marker,
            empty_shape,
        })
    }

    pub(crate) const fn horizontal_align(self) -> Option<&'static str> {
        self.marker.horizontal_align.xml_value()
    }

    pub(crate) const fn autofill(self) -> bool {
        self.marker.autofill
    }

    pub(crate) const fn is_empty_shape(self) -> bool {
        self.empty_shape
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootAutoCommandBarSchema {
    marker: Option<FormAutoCommandBarMarker>,
}

impl FormRootAutoCommandBarSchema {
    pub(crate) fn from_raw_layout(wrapper: &str, item_id: &str, fields: &[&str]) -> Option<Self> {
        if wrapper != "22" || item_id != "-1" {
            return None;
        }
        let marker = match fields.get(FormNestedAutoCommandBarSchema::MARKER_SLOT) {
            Some(raw) => Some(
                parse_root_auto_command_bar_marker(raw)
                    .or_else(|| parse_nested_auto_command_bar_marker(raw))?,
            ),
            None => None,
        };
        Some(Self { marker })
    }

    pub(crate) fn display_importance(self, fields: &[&str]) -> Option<&'static str> {
        FormChildItemDisplayImportanceSchema::from_raw_layout(
            "22",
            fields.len(),
            "AutoCommandBar",
            0,
        )
        .and_then(|schema| schema.display_importance(fields))
    }

    pub(crate) const fn horizontal_align(self) -> Option<&'static str> {
        match self.marker {
            Some(marker) => marker.horizontal_align.xml_value(),
            None => None,
        }
    }

    pub(crate) const fn autofill(self) -> Option<bool> {
        match self.marker {
            Some(marker) => Some(marker.autofill),
            None => None,
        }
    }
}

fn parse_auto_command_bar_marker_fields<const N: usize>(raw: &str) -> Option<[&str; N]> {
    if raw.len() > FormNestedAutoCommandBarSchema::MAX_MARKER_BYTES {
        return None;
    }
    let raw = raw.trim();
    let inner = raw.strip_prefix('{')?.strip_suffix('}')?;
    if inner.contains(['{', '}']) {
        return None;
    }
    let mut raw_fields = inner.split(',').map(str::trim);
    let mut fields = [""; N];
    for field in &mut fields {
        *field = raw_fields.next()?;
    }
    raw_fields.next().is_none().then_some(fields)
}

fn parse_nested_auto_command_bar_marker(raw: &str) -> Option<FormAutoCommandBarMarker> {
    let marker = parse_auto_command_bar_marker_fields::<3>(raw)?;
    if marker[0] != "0" {
        return None;
    }
    let horizontal_align = match marker[1] {
        "0" => FormAutoCommandBarHorizontalAlign::Default,
        "1" => FormAutoCommandBarHorizontalAlign::Center,
        "2" => FormAutoCommandBarHorizontalAlign::Right,
        "3" => FormAutoCommandBarHorizontalAlign::Auto,
        _ => return None,
    };
    let autofill = match marker[2] {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    Some(FormAutoCommandBarMarker {
        horizontal_align,
        autofill,
    })
}

fn parse_root_auto_command_bar_marker(raw: &str) -> Option<FormAutoCommandBarMarker> {
    let marker = parse_auto_command_bar_marker_fields::<4>(raw)?;
    if marker[0] != "1" || marker[3] != "0" {
        return None;
    }
    let horizontal_align = match marker[1] {
        "0" => FormAutoCommandBarHorizontalAlign::Default,
        "1" => FormAutoCommandBarHorizontalAlign::Center,
        "2" => FormAutoCommandBarHorizontalAlign::Right,
        "3" => FormAutoCommandBarHorizontalAlign::Auto,
        _ => return None,
    };
    let autofill = match marker[2] {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    Some(FormAutoCommandBarMarker {
        horizontal_align,
        autofill,
    })
}

#[cfg(test)]
mod nested_auto_command_bar_tests {
    use super::*;

    fn fixture(marker: &str, field_count: usize) -> Vec<&str> {
        let mut fields = vec!["0"; field_count];
        fields[FormNestedAutoCommandBarSchema::MARKER_SLOT] = marker;
        fields
    }

    fn parse(
        fields: &[&str],
        discriminator: Option<&str>,
    ) -> Option<FormNestedAutoCommandBarSchema> {
        FormNestedAutoCommandBarSchema::from_raw_layout(
            "22",
            "AutoCommandBar",
            "58",
            discriminator,
            fields,
        )
    }

    #[test]
    fn extraction_autofill_and_empty_shape_fixtures_are_typed() {
        let empty = parse(&fixture("{0,0,1}", 29), Some("9")).unwrap();
        assert_eq!(empty.horizontal_align(), None);
        assert!(empty.autofill());
        assert!(empty.is_empty_shape());

        let configured = parse(&fixture("{0,2,0}", 31), Some("9")).unwrap();
        assert_eq!(configured.horizontal_align(), Some("Right"));
        assert!(!configured.autofill());
        assert!(!configured.is_empty_shape());
    }

    #[test]
    fn discriminator_length_slot_and_enum_near_misses_fail_closed() {
        assert!(parse(&fixture("{0,0,1}", 29), Some("8")).is_none());
        assert!(parse(&fixture("{0,0,1}", 28), Some("9")).is_none());
        assert!(parse(&fixture("{0,0,1}", 30), Some("9")).is_none());
        for marker in [
            "{0,4,1}",
            "{0,0,2}",
            "{1,0,1}",
            "{0,0}",
            "{0,0,1,0}",
            "{0,0,1}tail",
        ] {
            assert!(parse(&fixture(marker, 29), Some("9")).is_none(), "{marker}");
        }
        let mut wrong_slot = fixture("{0,0,1}", 29);
        wrong_slot[FormNestedAutoCommandBarSchema::MARKER_SLOT] = "0";
        wrong_slot[19] = "{0,0,1}";
        assert!(parse(&wrong_slot, Some("9")).is_none());

        let huge_comma_marker = format!("{{0,0,1{}}}", ",".repeat(129));
        assert!(parse(&fixture(&huge_comma_marker, 29), Some("9")).is_none());
        let huge_whitespace_marker = format!("{{0,{},1}}", " ".repeat(129));
        assert!(parse(&fixture(&huge_whitespace_marker, 29), Some("9")).is_none());
        let huge_outer_whitespace_marker = format!("{}{{0,0,1}}", " ".repeat(129));
        assert!(parse(&fixture(&huge_outer_whitespace_marker, 29), Some("9")).is_none());
    }

    #[test]
    fn root_profile_marker_is_typed_and_present_malformed_marker_is_rejected() {
        let fields = fixture("{1,2,0,0}", 29);
        let schema = FormRootAutoCommandBarSchema::from_raw_layout("22", "-1", &fields).unwrap();
        assert_eq!(schema.horizontal_align(), Some("Right"));
        assert_eq!(schema.autofill(), Some(false));

        let platform_8_3_27_fields = fixture("{0,0,1}", 29);
        let platform_8_3_27_schema =
            FormRootAutoCommandBarSchema::from_raw_layout("22", "-1", &platform_8_3_27_fields)
                .unwrap();
        assert_eq!(platform_8_3_27_schema.horizontal_align(), None);
        assert_eq!(platform_8_3_27_schema.autofill(), Some(true));

        for marker in ["{1,4,0,0}", "{1,2,2,0}", "{0,2,0,0}", "{1,2,0}"] {
            let fields = fixture(marker, 29);
            assert!(
                FormRootAutoCommandBarSchema::from_raw_layout("22", "-1", &fields).is_none(),
                "{marker}"
            );
        }
        let huge_comma_marker = format!("{{1,2,0,0{}}}", ",".repeat(129));
        assert!(
            FormRootAutoCommandBarSchema::from_raw_layout(
                "22",
                "-1",
                &fixture(&huge_comma_marker, 29),
            )
            .is_none()
        );
        let huge_whitespace_marker = format!("{{1,{},0,0}}", " ".repeat(129));
        assert!(
            FormRootAutoCommandBarSchema::from_raw_layout(
                "22",
                "-1",
                &fixture(&huge_whitespace_marker, 29),
            )
            .is_none()
        );
        assert!(FormRootAutoCommandBarSchema::from_raw_layout("22", "58", &fields).is_none());
    }

    #[test]
    fn root_profile_preserves_genuinely_absent_marker_semantics() {
        let fields = vec!["0"; FormNestedAutoCommandBarSchema::MARKER_SLOT];
        let schema = FormRootAutoCommandBarSchema::from_raw_layout("22", "-1", &fields).unwrap();
        assert_eq!(schema.horizontal_align(), None);
        assert_eq!(schema.autofill(), None);
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
    Shortcut,
    TitleTextColor,
    TitleFont,
    ToolTip,
    ToolTipRepresentation,
}

pub(crate) const FORM_USUAL_GROUP_HEADER_XML_ORDER: &[FormUsualGroupHeaderXmlProperty] = &[
    FormUsualGroupHeaderXmlProperty::Title,
    FormUsualGroupHeaderXmlProperty::Shortcut,
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
    AfterShowTitle,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormUsualGroupXmlProperty {
    ReadOnly,
    Enabled,
    EnableContentChange,
    GroupHorizontalAlign,
    GroupVerticalAlign,
    ChildrenAlign,
    HorizontalSpacing,
    VerticalSpacing,
    HorizontalAlign,
    VerticalAlign,
    CollapsedRepresentationTitle,
    Collapsed,
    ControlRepresentation,
    Format,
    ShowLeftMargin,
    United,
    ChildItemsWidth,
    BackColor,
    ThroughAlign,
}

pub(crate) const FORM_USUAL_GROUP_XML_ORDER: &[FormUsualGroupXmlProperty] = &[
    FormUsualGroupXmlProperty::ReadOnly,
    FormUsualGroupXmlProperty::Enabled,
    FormUsualGroupXmlProperty::EnableContentChange,
    FormUsualGroupXmlProperty::GroupHorizontalAlign,
    FormUsualGroupXmlProperty::GroupVerticalAlign,
    FormUsualGroupXmlProperty::ChildrenAlign,
    FormUsualGroupXmlProperty::HorizontalSpacing,
    FormUsualGroupXmlProperty::VerticalSpacing,
    FormUsualGroupXmlProperty::HorizontalAlign,
    FormUsualGroupXmlProperty::VerticalAlign,
    FormUsualGroupXmlProperty::CollapsedRepresentationTitle,
    FormUsualGroupXmlProperty::Collapsed,
    FormUsualGroupXmlProperty::ControlRepresentation,
    FormUsualGroupXmlProperty::Format,
    FormUsualGroupXmlProperty::ShowLeftMargin,
    FormUsualGroupXmlProperty::United,
    FormUsualGroupXmlProperty::ChildItemsWidth,
    FormUsualGroupXmlProperty::BackColor,
    FormUsualGroupXmlProperty::ThroughAlign,
];

impl FormUsualGroupXmlProperty {
    pub(crate) const fn anchor(self) -> FormUsualGroupXmlAnchor {
        match self {
            Self::ReadOnly | Self::Enabled | Self::EnableContentChange => {
                FormUsualGroupXmlAnchor::BeforeTitle
            }
            Self::GroupHorizontalAlign | Self::GroupVerticalAlign => {
                FormUsualGroupXmlAnchor::BeforeGroup
            }
            Self::ChildrenAlign
            | Self::HorizontalSpacing
            | Self::VerticalSpacing
            | Self::HorizontalAlign
            | Self::VerticalAlign => FormUsualGroupXmlAnchor::BeforeBehavior,
            Self::CollapsedRepresentationTitle | Self::Collapsed | Self::ControlRepresentation => {
                FormUsualGroupXmlAnchor::AfterBehavior
            }
            Self::Format | Self::ShowLeftMargin | Self::United | Self::ChildItemsWidth => {
                FormUsualGroupXmlAnchor::AfterRepresentation
            }
            Self::BackColor | Self::ThroughAlign => FormUsualGroupXmlAnchor::AfterShowTitle,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormUsualGroupSchema;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormUsualGroupProperties {
    enabled: Option<bool>,
    read_only: Option<bool>,
    enable_content_change: Option<bool>,
    group_horizontal_align: Option<&'static str>,
    group_vertical_align: Option<FormUsualGroupGroupVerticalAlign>,
    children_align: Option<&'static str>,
    horizontal_spacing: Option<&'static str>,
    vertical_spacing: Option<&'static str>,
    child_items_width: Option<&'static str>,
    control_representation: Option<&'static str>,
    collapsed: Option<bool>,
    horizontal_align: Option<&'static str>,
    vertical_align: Option<&'static str>,
    through_align: Option<&'static str>,
    united: Option<bool>,
    show_left_margin: Option<bool>,
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
    const GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET: usize = 3;
    const GROUP_VERTICAL_ALIGN_REVERSE_OFFSET: usize = 2;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        (matches!(
            field_count,
            30 | 32 | 34 | 36 | 38 | 40 | 42 | 44 | 46 | 48 | 50 | 52 | 54 | 60
        ) && matches!(
            (
                wrapper,
                item_tag,
                direct_discriminator,
                options.len(),
                options.first().map(|field| field.trim()),
            ),
            ("22", "UsualGroup", Some("5"), 29, Some("29"))
        ))
        .then_some(Self)
    }

    pub(crate) fn properties(self, fields: &[&str], options: &[&str]) -> FormUsualGroupProperties {
        FormUsualGroupProperties {
            enabled: (fields.get(10).map(|field| field.trim()) == Some("0")).then_some(false),
            read_only: (fields.get(11).map(|field| field.trim()) == Some("1")).then_some(true),
            enable_content_change: (fields.get(9).map(|field| field.trim()) == Some("1"))
                .then_some(true),
            group_horizontal_align: self.group_horizontal_align(fields),
            group_vertical_align: self.group_vertical_align(fields),
            children_align: options.get(20).and_then(|field| match field.trim() {
                "1" => Some("None"),
                "2" => Some("ItemsLeftTitlesLeft"),
                "3" => Some("ItemsRightTitlesLeft"),
                "6" => Some("TitlesLeftDataAuto"),
                _ => None,
            }),
            horizontal_spacing: options.get(15).and_then(|field| match field.trim() {
                "1" => Some("None"),
                "2" => Some("Half"),
                "3" => Some("Single"),
                "5" => Some("Double"),
                _ => None,
            }),
            vertical_spacing: options.get(16).and_then(|field| match field.trim() {
                "1" => Some("None"),
                "2" => Some("Half"),
                "4" => Some("OneAndHalf"),
                _ => None,
            }),
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
            show_left_margin: (options.get(13).map(|field| field.trim()) == Some("0"))
                .then_some(false),
        }
    }

    pub(crate) fn height(self, fields: &[&str]) -> Option<String> {
        let value = fields.get(13)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }

    pub(crate) fn shortcut_field<'a>(self, fields: &'a [&'a str]) -> Option<&'a str> {
        fields.get(18).copied()
    }

    pub(crate) fn format_field<'a>(self, options: &'a [&'a str]) -> Option<&'a str> {
        options.get(6).copied()
    }

    pub(crate) fn collapsed_representation_title_field<'a>(
        self,
        options: &'a [&'a str],
    ) -> Option<&'a str> {
        options.get(14).copied()
    }

    fn group_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        let slot = fields
            .len()
            .checked_sub(Self::GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "0" => Some("Left"),
            "1" => Some("Center"),
            "2" => Some("Right"),
            _ => None,
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
    pub(crate) const fn enabled(self) -> Option<bool> {
        self.enabled
    }

    pub(crate) const fn read_only(self) -> Option<bool> {
        self.read_only
    }

    pub(crate) const fn enable_content_change(self) -> Option<bool> {
        self.enable_content_change
    }

    pub(crate) const fn group_horizontal_align(self) -> Option<&'static str> {
        self.group_horizontal_align
    }

    pub(crate) const fn group_vertical_align(self) -> Option<FormUsualGroupGroupVerticalAlign> {
        self.group_vertical_align
    }

    pub(crate) const fn child_items_width(self) -> Option<&'static str> {
        self.child_items_width
    }

    pub(crate) const fn children_align(self) -> Option<&'static str> {
        self.children_align
    }

    pub(crate) const fn horizontal_spacing(self) -> Option<&'static str> {
        self.horizontal_spacing
    }

    pub(crate) const fn vertical_spacing(self) -> Option<&'static str> {
        self.vertical_spacing
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

    pub(crate) const fn show_left_margin(self) -> Option<bool> {
        self.show_left_margin
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
    Hyperlink,
    GroupHorizontalAlign,
    VerticalAlign,
    Events,
}

/// `Hyperlink` follows `Title` and precedes `Events`.  Only two of the 8 native
/// `ExtendedTooltip` items that carry it carry anything else, and they pin the
/// property from both sides with no counter-example: one reads `TextColor`,
/// `Title`, `Hyperlink` and the other `TextColor`, `Hyperlink`, `Events`.  Its
/// position relative to `GroupHorizontalAlign` and `VerticalAlign` is
/// unobserved, since it never co-occurs with either.
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
    FormExtendedTooltipXmlProperty::Hyperlink,
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
    /// `Hyperlink` flag slot of the tooltip option tuple: `1` on exactly the 8
    /// native `ExtendedTooltip` items that carry `<Hyperlink>true</Hyperlink>`
    /// and `0` on the other 170 224.
    pub(crate) const HYPERLINK_OPTION_SLOT: usize = 1;

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
pub(crate) enum FormControlBorderStyle {
    WithoutBorder,
    Single,
    Underline,
    Overline,
}

impl FormControlBorderStyle {
    pub(crate) fn from_raw_code(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::WithoutBorder),
            "1" => Some(Self::Single),
            "4" => Some(Self::Underline),
            "7" => Some(Self::Overline),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::WithoutBorder => "0",
            Self::Single => "1",
            Self::Underline => "4",
            Self::Overline => "7",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value.trim() {
            "WithoutBorder" => Some(Self::WithoutBorder),
            "Single" => Some(Self::Single),
            "Underline" => Some(Self::Underline),
            "Overline" => Some(Self::Overline),
            _ => None,
        }
    }

    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::WithoutBorder => "WithoutBorder",
            Self::Single => "Single",
            Self::Underline => "Underline",
            Self::Overline => "Overline",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormControlBorderSchema {
    border_option_slot: usize,
    default_style: FormControlBorderStyle,
}

impl FormControlBorderSchema {
    pub(crate) fn options_slot(item_tag: &str, top_level_offset: usize) -> Option<usize> {
        match item_tag {
            "LabelField" | "PictureField" if top_level_offset <= 1 => {
                Some(FormFieldSchema::OPTIONS_BASE_SLOT + top_level_offset)
            }
            "LabelDecoration" | "PictureDecoration" if top_level_offset == 0 => Some(18),
            _ => None,
        }
    }

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        let (border_option_slot, default_style) = match (
            wrapper,
            field_count,
            item_tag,
            top_level_offset,
            direct_discriminator,
            options.len(),
            options.first().map(|field| field.trim()),
        ) {
            ("37", 59, "LabelField", 0, Some("1"), 20, Some("11"))
            | ("37", 60, "LabelField", 1, Some("1"), 20, Some("11")) => {
                (14, FormControlBorderStyle::WithoutBorder)
            }
            ("37", 59, "PictureField", 0, Some("4"), 24, Some("10"))
            | ("37", 60, "PictureField", 1, Some("4"), 24, Some("10")) => {
                (13, FormControlBorderStyle::Single)
            }
            ("12", 36, "LabelDecoration", 0, Some("0"), 9, Some("5")) => {
                (8, FormControlBorderStyle::WithoutBorder)
            }
            ("12", 36, "PictureDecoration", 0, Some("1"), 13, Some("4")) => {
                (7, FormControlBorderStyle::WithoutBorder)
            }
            _ => return None,
        };
        Some(Self {
            border_option_slot,
            default_style,
        })
    }

    pub(crate) const fn border_option_slot(self) -> usize {
        self.border_option_slot
    }

    pub(crate) fn tuple_style(self, tuple: &[&str]) -> Option<FormControlBorderStyle> {
        if tuple.len() != 7
            || tuple.first().map(|field| field.trim()) != Some("3")
            || tuple.get(1).map(|field| field.trim()) != Some("0")
            || tuple.get(2).map(|field| field.trim()) != Some("{0}")
            || tuple.get(4).map(|field| field.trim()) != Some("1")
            || tuple.get(5).map(|field| field.trim()) != Some("0")
            || uuid::Uuid::parse_str(tuple.get(6)?.trim()).is_err()
        {
            return None;
        }
        FormControlBorderStyle::from_raw_code(tuple.get(3)?)
    }

    pub(crate) fn non_default_tuple_style(self, tuple: &[&str]) -> Option<FormControlBorderStyle> {
        self.tuple_style(tuple)
            .filter(|style| *style != self.default_style)
    }
}

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
    pub(crate) const OPTIONS_SLOT: usize = 18;

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

    pub(crate) fn hyperlink(self, options: &[&str]) -> Option<bool> {
        self.hyperlink_option_slot(options)?;
        (options[2].trim() == "1").then_some(true)
    }

    pub(crate) fn hyperlink_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(2)
    }

    /// Picture-size code slot of the decoration option tuple.  Over all 3 666
    /// native `PictureDecoration` items the slot is a total function of the
    /// native spelling: `0` with no `<PictureSize>` (3 281), `2` ->
    /// `Proportionally` (161), `1` -> `Stretch` (138), `4` -> `AutoSize` (58),
    /// `7` -> `ByFontSize` (25), `6` -> `AutoSizeIgnoreScale` (2) and `5` ->
    /// `RealSizeIgnoreScale` (1).  Every code shared with the `PictureField`
    /// tuple maps to the same spelling in both, so the two owners read one
    /// table, not two.
    pub(crate) fn picture_size_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(3)
    }

    /// `NonselectedPictureText` slot of the decoration option tuple: the empty
    /// localised-string record `{1,0}` on exactly the 3 650 native
    /// `PictureDecoration` items without the property and a populated record on
    /// the other 16.
    pub(crate) fn nonselected_picture_text_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(5)
    }

    /// The option tuple every native `PictureDecoration` carries: 13 slots
    /// discriminated by a leading `4`, with slot 2 a plain hyperlink flag.
    fn option_tuple_is_exact(self, options: &[&str]) -> bool {
        options.len() == 13
            && options.first().map(|field| field.trim()) == Some("4")
            && matches!(options.get(2).map(|field| field.trim()), Some("0" | "1"))
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
pub(crate) struct FormChildItemDisplayImportanceSchema {
    slot: usize,
}

impl FormChildItemDisplayImportanceSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
    ) -> Option<Self> {
        let slot = match (wrapper, field_count, item_tag, top_level_offset) {
            (
                "22",
                field_count,
                "CommandBar" | "Popup" | "ColumnGroup" | "Pages" | "Page" | "UsualGroup"
                | "ButtonGroup" | "AutoCommandBar",
                0,
            ) if field_count >= 29 => field_count.checked_sub(1)?,
            ("12", 36, "LabelDecoration" | "PictureDecoration", 0) => 34,
            ("31", 52, "Button", 0) | ("31", 53, "Button", 1) => field_count.checked_sub(4)?,
            (
                "37",
                59,
                "LabelField"
                | "InputField"
                | "CheckBoxField"
                | "PictureField"
                | "RadioButtonField"
                | "SpreadSheetDocumentField"
                | "TextDocumentField"
                | "CalendarField"
                | "GraphicalSchemaField"
                | "HTMLDocumentField"
                | "FormattedDocumentField"
                | "ProgressBarField"
                | "TrackBarField"
                | "ChartField",
                0,
            )
            | ("37", 60, "LabelField" | "InputField" | "CheckBoxField" | "PictureField", 1) => {
                field_count.checked_sub(4)?
            }
            ("55", field_count, "Table", 0) if field_count >= 99 && (field_count - 99) % 2 == 0 => {
                field_count.checked_sub(3)?
            }
            _ => return None,
        };
        Some(Self { slot })
    }

    pub(crate) fn display_importance(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(self.slot)?.trim() {
            "1" => Some("VeryHigh"),
            "2" => Some("High"),
            "3" => Some("Usual"),
            "4" => Some("Low"),
            "5" => Some("VeryLow"),
            _ => None,
        }
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
pub(crate) struct FormFieldTitleLocationSchema {
    slot: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormWarningOnEditRepresentation {
    Show,
    DontShow,
}

impl FormWarningOnEditRepresentation {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Show => "Show",
            Self::DontShow => "DontShow",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "Show" => Some(Self::Show),
            "DontShow" => Some(Self::DontShow),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Show => "0",
            Self::DontShow => "1",
        }
    }
}

impl FormFieldTitleLocationSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
        direct_discriminator: Option<&str>,
    ) -> Option<Self> {
        let discriminator = match item_tag {
            "LabelField" => "1",
            "InputField" => "2",
            "CheckBoxField" => "3",
            "PictureField" => "4",
            "RadioButtonField" => "5",
            "SpreadSheetDocumentField" => "6",
            "TextDocumentField" => "7",
            "CalendarField" => "8",
            "ProgressBarField" => "9",
            "TrackBarField" => "10",
            "ChartField" => "11",
            "GraphicalSchemaField" => "14",
            "HTMLDocumentField" => "15",
            "FormattedDocumentField" => "17",
            _ => return None,
        };
        if !matches!(wrapper, "37" | "48")
            || field_count <= 20
            || top_level_offset > 1
            || direct_discriminator != Some(discriminator)
        {
            return None;
        }
        Some(Self {
            slot: 7 + top_level_offset,
        })
    }

    pub(crate) fn title_location(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(self.slot)?.trim() {
            "0" => Some("None"),
            "2" => Some("Left"),
            "3" => Some("Top"),
            "4" => Some("Right"),
            _ => None,
        }
    }

    pub(crate) fn follows_title_in_xml(item_tag: &str, has_title: bool) -> bool {
        item_tag == "FormattedDocumentField" && has_title
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFieldGroupHorizontalAlign {
    Left,
    Center,
    Right,
}

impl FormFieldGroupHorizontalAlign {
    pub(crate) fn from_raw_value(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::Left),
            "1" => Some(Self::Center),
            "2" => Some(Self::Right),
            _ => None,
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value.trim() {
            "Left" => Some(Self::Left),
            "Center" => Some(Self::Center),
            "Right" => Some(Self::Right),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Left => "0",
            Self::Center => "1",
            Self::Right => "2",
        }
    }

    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFixingInTable {
    Left,
    Right,
}

impl FormFixingInTable {
    pub(crate) fn from_field_raw_value(value: &str) -> Option<Self> {
        match value.trim() {
            "1" => Some(Self::Left),
            "2" => Some(Self::Right),
            _ => None,
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value.trim() {
            "Left" => Some(Self::Left),
            "Right" => Some(Self::Right),
            _ => None,
        }
    }

    pub(crate) const fn field_raw_code(self) -> &'static str {
        match self {
            Self::Left => "1",
            Self::Right => "2",
        }
    }

    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormColumnGroupSchema;

impl FormColumnGroupSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;
    pub(crate) const FIXING_IN_TABLE_OPTION_SLOT: usize = 11;

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
            && item_tag == "ColumnGroup"
            && direct_discriminator == Some("2")
            && options.len() == 12
            && options.first().map(|field| field.trim()) == Some("2")
            && matches!(
                options
                    .get(Self::FIXING_IN_TABLE_OPTION_SLOT)
                    .map(|field| field.trim()),
                Some("0" | "1")
            ))
        .then_some(Self)
    }

    pub(crate) fn fixing_in_table(self, options: &[&str]) -> Option<FormFixingInTable> {
        (options.get(Self::FIXING_IN_TABLE_OPTION_SLOT)?.trim() == "1")
            .then_some(FormFixingInTable::Left)
    }

    pub(crate) const fn fixing_in_table_raw_code(
        self,
        value: FormFixingInTable,
    ) -> Option<&'static str> {
        match value {
            FormFixingInTable::Left => Some("1"),
            FormFixingInTable::Right => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFieldVerticalAlign {
    Top,
    Center,
    Bottom,
}

impl FormFieldVerticalAlign {
    pub(crate) fn from_raw_value(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::Top),
            "1" => Some(Self::Center),
            "2" => Some(Self::Bottom),
            _ => None,
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value.trim() {
            "Top" => Some(Self::Top),
            "Center" => Some(Self::Center),
            "Bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Top => "0",
            Self::Center => "1",
            Self::Bottom => "2",
        }
    }

    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Center => "Center",
            Self::Bottom => "Bottom",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormFieldSchema {
    top_level_offset: usize,
    input_field_options: bool,
    spreadsheet_document_options: bool,
    title_slot: usize,
    width_option_slot: Option<usize>,
    height_option_slot: Option<usize>,
    horizontal_stretch_option_slot: Option<usize>,
    vertical_stretch_option_slot: Option<usize>,
    show_in_header_slot: Option<usize>,
    auto_cell_height_slot: Option<usize>,
    cell_hyperlink_slot: Option<usize>,
    show_in_footer_slot: Option<usize>,
    read_only_slot: Option<usize>,
    title_height_slot: Option<usize>,
    horizontal_align_slot: Option<usize>,
    fixing_in_table_slot: Option<usize>,
    enabled_slot: Option<usize>,
    text_color_option_slot: Option<usize>,
    back_color_option_slot: Option<usize>,
    border_color_option_slot: Option<usize>,
    extended_edit_multiple_values_option_slot: Option<usize>,
    picture_size_option_slot: Option<usize>,
    hyperlink_option_slot: Option<usize>,
    nonselected_picture_text_option_slot: Option<usize>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormSpreadsheetDocumentFieldProperties {
    pub(crate) default_item: Option<bool>,
    pub(crate) width: Option<String>,
    pub(crate) height: Option<String>,
    pub(crate) auto_max_width: Option<bool>,
    pub(crate) auto_max_height: Option<bool>,
    pub(crate) vertical_stretch: Option<bool>,
    pub(crate) show_grid: Option<bool>,
    pub(crate) show_headers: Option<bool>,
    pub(crate) show_cell_names: Option<bool>,
    pub(crate) show_row_and_column_names: Option<bool>,
    pub(crate) vertical_scroll_bar: Option<bool>,
    pub(crate) horizontal_scroll_bar: Option<bool>,
    pub(crate) edit: Option<bool>,
    pub(crate) selection_show_mode: Option<&'static str>,
    pub(crate) output: Option<&'static str>,
    pub(crate) protection: Option<bool>,
    pub(crate) enable_start_drag: Option<bool>,
    pub(crate) enable_drag: Option<bool>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormChildItemEventCollectionOwner {
    LabelField,
    PictureField,
    SpreadSheetDocumentField,
    CalendarField,
    GraphicalSchemaField,
    Pages,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormChildItemEventCollectionSchema {
    owner: FormChildItemEventCollectionOwner,
    collection_slot: usize,
}

// Platform event type IDs stored in the strict managed-form event collections below.
const FORM_LABEL_FIELD_CLICK_EVENT_UUID: &str = "eba5f295-c611-4dd9-84b5-22911ad60c53";
const FORM_LABEL_FIELD_URL_PROCESSING_EVENT_UUID: &str = "509eca20-d6e4-4fef-a0f8-3a6b44c64178";
const FORM_PICTURE_FIELD_CLICK_EVENT_UUID: &str = "996b8c30-7a89-4973-8d56-2c9ce2976695";
const FORM_SPREADSHEET_ADDITIONAL_DETAIL_PROCESSING_EVENT_UUID: &str =
    "0b8dc702-d001-4637-a215-9f35613e096c";
const FORM_SPREADSHEET_BEFORE_WRITE_EVENT_UUID: &str = "b7646583-04d3-4905-8f04-8985914bd1b7";
const FORM_SPREADSHEET_DETAIL_PROCESSING_EVENT_UUID: &str = "2988b2a5-c887-4928-94ae-5d0c9c31e999";
const FORM_SPREADSHEET_DRAG_EVENT_UUID: &str = "8ad48496-8d0b-4f6c-ae48-99d95227884b";
const FORM_SPREADSHEET_DRAG_CHECK_EVENT_UUID: &str = "0d644ff6-443b-4390-86fa-7f9105e42711";
const FORM_SPREADSHEET_ON_ACTIVATE_EVENT_UUID: &str = "2042ec93-3108-4190-b767-ec6c10dd9ff4";
const FORM_SPREADSHEET_ON_CHANGE_AREA_CONTENT_EVENT_UUID: &str =
    "411a4578-276c-4f4a-b56a-b3b01181c997";
const FORM_SPREADSHEET_SELECTION_EVENT_UUID: &str = "22287505-97d8-4258-a318-209e2493f7eb";
const FORM_CALENDAR_ON_PERIOD_OUTPUT_EVENT_UUID: &str = "1490ede6-6f33-4c6d-b971-53b2541331ea";
const FORM_CALENDAR_SELECTION_EVENT_UUID: &str = "2feb1ee9-b750-4352-bb4c-67ba1c608dc6";
const FORM_GRAPHICAL_SCHEMA_SELECTION_EVENT_UUID: &str = "3c3da18f-fc18-4f77-8c2d-96c25bec40a5";
const FORM_PAGES_CURRENT_PAGE_CHANGE_EVENT_UUID: &str = "526c501f-ed3f-4db4-8731-fd0324707501";

impl FormChildItemEventCollectionSchema {
    pub(crate) fn from_field_schema(
        _field_schema: FormFieldSchema,
        item_tag: &str,
    ) -> Option<Self> {
        let (owner, collection_slot) = match item_tag {
            "LabelField" => (FormChildItemEventCollectionOwner::LabelField, 12),
            "PictureField" => (FormChildItemEventCollectionOwner::PictureField, 16),
            "SpreadSheetDocumentField" => (
                FormChildItemEventCollectionOwner::SpreadSheetDocumentField,
                18,
            ),
            "CalendarField" => (FormChildItemEventCollectionOwner::CalendarField, 14),
            "GraphicalSchemaField" => (FormChildItemEventCollectionOwner::GraphicalSchemaField, 6),
            _ => return None,
        };
        Some(Self {
            owner,
            collection_slot,
        })
    }

    pub(crate) fn from_pages_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        container: &[&str],
    ) -> Option<Self> {
        (wrapper == "22"
            && field_count >= 30
            && (field_count - 30) % 2 == 0
            && item_tag == "Pages"
            && direct_discriminator == Some("3")
            && container.len() == 6
            && container.first().map(|field| field.trim()) == Some("4")
            && matches!(
                container.get(1).map(|field| field.trim()),
                Some("0" | "1" | "2" | "3" | "5")
            )
            && container.get(3).map(|field| field.trim()) == Some("2")
            && container.get(4).map(|field| field.trim()) == Some("0")
            && container.get(5).map(|field| field.trim())
                == container.get(1).map(|field| field.trim()))
        .then_some(Self {
            owner: FormChildItemEventCollectionOwner::Pages,
            collection_slot: 2,
        })
    }

    pub(crate) const fn collection_slot(self) -> usize {
        self.collection_slot
    }

    pub(crate) fn event_name(self, event_id: &str) -> Option<&'static str> {
        let mappings: &[(&str, &str)] = match self.owner {
            FormChildItemEventCollectionOwner::LabelField => &[
                (FORM_LABEL_FIELD_CLICK_EVENT_UUID, "Click"),
                (FORM_LABEL_FIELD_URL_PROCESSING_EVENT_UUID, "URLProcessing"),
            ],
            FormChildItemEventCollectionOwner::PictureField => {
                &[(FORM_PICTURE_FIELD_CLICK_EVENT_UUID, "Click")]
            }
            FormChildItemEventCollectionOwner::SpreadSheetDocumentField => &[
                (
                    FORM_SPREADSHEET_ADDITIONAL_DETAIL_PROCESSING_EVENT_UUID,
                    "AdditionalDetailProcessing",
                ),
                (FORM_SPREADSHEET_BEFORE_WRITE_EVENT_UUID, "BeforeWrite"),
                (
                    FORM_SPREADSHEET_DETAIL_PROCESSING_EVENT_UUID,
                    "DetailProcessing",
                ),
                (FORM_SPREADSHEET_DRAG_EVENT_UUID, "Drag"),
                (FORM_SPREADSHEET_DRAG_CHECK_EVENT_UUID, "DragCheck"),
                (FORM_SPREADSHEET_ON_ACTIVATE_EVENT_UUID, "OnActivate"),
                (
                    FORM_SPREADSHEET_ON_CHANGE_AREA_CONTENT_EVENT_UUID,
                    "OnChangeAreaContent",
                ),
                (FORM_SPREADSHEET_SELECTION_EVENT_UUID, "Selection"),
            ],
            FormChildItemEventCollectionOwner::CalendarField => &[
                (FORM_CALENDAR_ON_PERIOD_OUTPUT_EVENT_UUID, "OnPeriodOutput"),
                (FORM_CALENDAR_SELECTION_EVENT_UUID, "Selection"),
            ],
            FormChildItemEventCollectionOwner::GraphicalSchemaField => {
                &[(FORM_GRAPHICAL_SCHEMA_SELECTION_EVENT_UUID, "Selection")]
            }
            FormChildItemEventCollectionOwner::Pages => &[(
                FORM_PAGES_CURRENT_PAGE_CHANGE_EVENT_UUID,
                "OnCurrentPageChange",
            )],
        };
        mappings
            .iter()
            .find_map(|(id, name)| id.eq_ignore_ascii_case(event_id).then_some(*name))
    }
}

impl FormFieldSchema {
    pub(crate) const OPTIONS_BASE_SLOT: usize = 39;

    pub(crate) const fn options_slot(self) -> usize {
        Self::OPTIONS_BASE_SLOT + self.top_level_offset
    }

    pub(crate) fn item_tag_from_discriminator(discriminator: &str) -> Option<&'static str> {
        match discriminator {
            "1" => Some("LabelField"),
            "2" => Some("InputField"),
            "3" => Some("CheckBoxField"),
            "4" => Some("PictureField"),
            "5" => Some("RadioButtonField"),
            "6" => Some("SpreadSheetDocumentField"),
            "7" => Some("TextDocumentField"),
            "8" => Some("CalendarField"),
            "14" => Some("GraphicalSchemaField"),
            "15" => Some("HTMLDocumentField"),
            "17" => Some("FormattedDocumentField"),
            _ => None,
        }
    }

    pub(crate) fn supports_item_tag(item_tag: &str) -> bool {
        matches!(
            item_tag,
            "LabelField"
                | "InputField"
                | "CheckBoxField"
                | "PictureField"
                | "RadioButtonField"
                | "SpreadSheetDocumentField"
                | "TextDocumentField"
                | "CalendarField"
                | "GraphicalSchemaField"
                | "HTMLDocumentField"
                | "FormattedDocumentField"
        )
    }

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
            // Option slot 10 holds the text colour on all 2 212 native
            // `PictureField` items: unset on the 2 195 without a `<TextColor>`,
            // readable on all 17 that carry one.
            "PictureField" => ("4", 24, "10", Some(10), None, None),
            // Likewise option slot 3 on all 1 381 native `RadioButtonField`
            // items.
            "RadioButtonField" => ("5", 12, "8", Some(3), None, None),
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
            input_field_options: item_tag == "InputField",
            spreadsheet_document_options: item_tag == "SpreadSheetDocumentField",
            title_slot: 9 + top_level_offset,
            width_option_slot: (item_tag == "PictureField").then_some(1),
            height_option_slot: (item_tag == "PictureField").then_some(2),
            horizontal_stretch_option_slot: (item_tag == "PictureField").then_some(3),
            vertical_stretch_option_slot: (item_tag == "PictureField").then_some(4),
            show_in_header_slot: matches!(
                item_tag,
                "InputField" | "LabelField" | "CheckBoxField" | "PictureField"
            )
            .then_some(20 + top_level_offset),
            auto_cell_height_slot: matches!(item_tag, "InputField" | "LabelField" | "PictureField")
                .then_some(28 + top_level_offset),
            // Slot `22 + top_level_offset` carries `CellHyperlink` for every
            // field kind that can sit in a table cell, not only the two the
            // decoder used to admit: on the 1 181 nested `PictureField` items
            // the slot reads `1` on exactly the 47 that carry
            // `<CellHyperlink>true</CellHyperlink>` and `0` on the other 1 134,
            // and on the 4 687 nested `CheckBoxField` items on exactly the 3
            // that carry it.  The 12 top-level items of either kind read the
            // slot at the shifted index and never carry the property.
            cell_hyperlink_slot: matches!(
                item_tag,
                "InputField" | "LabelField" | "PictureField" | "CheckBoxField"
            )
            .then_some(22 + top_level_offset),
            show_in_footer_slot: matches!(item_tag, "InputField" | "LabelField" | "PictureField")
                .then_some(21 + top_level_offset),
            read_only_slot: matches!(
                item_tag,
                "InputField"
                    | "LabelField"
                    | "CheckBoxField"
                    | "PictureField"
                    | "SpreadSheetDocumentField"
                    | "FormattedDocumentField"
            )
            .then_some(14 + top_level_offset),
            title_height_slot: matches!(
                item_tag,
                "InputField" | "LabelField" | "CheckBoxField" | "PictureField" | "RadioButtonField"
            )
            .then_some(8 + top_level_offset),
            horizontal_align_slot: matches!(
                item_tag,
                "InputField" | "LabelField" | "CheckBoxField" | "PictureField"
            )
            .then_some(23 + top_level_offset),
            fixing_in_table_slot: matches!(
                item_tag,
                "InputField" | "LabelField" | "CheckBoxField" | "PictureField"
            )
            .then_some(49 + top_level_offset),
            enabled_slot: matches!(
                item_tag,
                "InputField" | "LabelField" | "CheckBoxField" | "PictureField"
            )
            .then_some(13 + top_level_offset),
            text_color_option_slot: text,
            back_color_option_slot: back,
            border_color_option_slot: border,
            extended_edit_multiple_values_option_slot: (item_tag == "InputField")
                .then_some(FormInputFieldExtendedOptionSlot::ExtendedEditMultipleValues.index()),
            // The `PictureField` option tuple (kind `10`, 24 slots) carries the
            // three picture properties in a fixed run: slot 6 the picture size
            // code, slot 8 the hyperlink flag, slot 9 the unselected-picture
            // caption.  Each is a total function of the native XML over all
            // 1 193 native `PictureField` items - see the accessors below.
            picture_size_option_slot: (item_tag == "PictureField").then_some(6),
            hyperlink_option_slot: (item_tag == "PictureField").then_some(8),
            nonselected_picture_text_option_slot: (item_tag == "PictureField").then_some(9),
        })
    }

    pub(crate) const fn title_slot(self) -> usize {
        self.title_slot
    }

    pub(crate) const fn tooltip_slot(self) -> usize {
        10 + self.top_level_offset
    }

    pub(crate) fn width(self, options: &[&str]) -> Option<String> {
        self.dimension(options, self.width_option_slot?)
    }

    pub(crate) fn height(self, options: &[&str]) -> Option<String> {
        self.dimension(options, self.height_option_slot?)
    }

    pub(crate) fn horizontal_stretch(self, options: &[&str]) -> Option<bool> {
        (options.get(self.horizontal_stretch_option_slot?)?.trim() == "0").then_some(false)
    }

    pub(crate) fn vertical_stretch(self, options: &[&str]) -> Option<bool> {
        (options.get(self.vertical_stretch_option_slot?)?.trim() == "0").then_some(false)
    }

    pub(crate) fn show_in_header(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.show_in_header_slot?)?.trim() == "0").then_some(false)
    }

    pub(crate) fn auto_cell_height(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.auto_cell_height_slot?)?.trim() == "1").then_some(true)
    }

    /// Picture-size code slot of the owning field's option tuple.
    ///
    /// Read on all 1 193 native `PictureField` items the slot takes six values,
    /// each mapping to exactly one native spelling and never to two: `0` with no
    /// `<PictureSize>` at all (1 055), `2` -> `Proportionally` (101), `4` ->
    /// `AutoSize` (29), `7` -> `ByFontSize` (5), `6` -> `AutoSizeIgnoreScale`
    /// (2) and `1` -> `Stretch` (1).
    pub(crate) const fn picture_size_option_slot(self) -> Option<usize> {
        self.picture_size_option_slot
    }

    /// `Hyperlink` flag slot of the owning field's option tuple: `1` on exactly
    /// the 51 native `PictureField` items that carry
    /// `<Hyperlink>true</Hyperlink>` and `0` on the other 1 142.
    pub(crate) fn hyperlink(self, options: &[&str]) -> Option<bool> {
        match options.get(self.hyperlink_option_slot?)?.trim() {
            "1" => Some(true),
            _ => None,
        }
    }

    /// `NonselectedPictureText` slot of the owning field's option tuple: the
    /// empty localised-string record `{1,0}` on exactly the 1 166 native
    /// `PictureField` items without the property and a populated record on the
    /// other 27.
    pub(crate) const fn nonselected_picture_text_option_slot(self) -> Option<usize> {
        self.nonselected_picture_text_option_slot
    }

    pub(crate) fn cell_hyperlink(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.cell_hyperlink_slot?)?.trim() {
            "1" => Some(true),
            "0" => None,
            _ => None,
        }
    }

    pub(crate) const fn cell_hyperlink_slot(self) -> Option<usize> {
        self.cell_hyperlink_slot
    }

    pub(crate) fn show_in_footer(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.show_in_footer_slot?)?.trim() {
            "0" => Some(false),
            "1" => None,
            _ => None,
        }
    }

    pub(crate) const fn show_in_footer_slot(self) -> Option<usize> {
        self.show_in_footer_slot
    }

    pub(crate) fn read_only(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.read_only_slot?)?.trim() == "1").then_some(true)
    }

    pub(crate) fn title_height(self, fields: &[&str]) -> Option<String> {
        self.dimension(fields, self.title_height_slot?)
    }

    pub(crate) fn horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(self.horizontal_align_slot?)?.trim() {
            "0" => Some("Left"),
            "1" => Some("Center"),
            "2" => Some("Right"),
            "3" => None,
            _ => None,
        }
    }

    pub(crate) const fn fixing_in_table_slot(self) -> Option<usize> {
        self.fixing_in_table_slot
    }

    pub(crate) fn fixing_in_table(self, fields: &[&str]) -> Option<FormFixingInTable> {
        FormFixingInTable::from_field_raw_value(fields.get(self.fixing_in_table_slot?)?)
    }

    pub(crate) const fn vertical_align_slot(self) -> usize {
        27 + self.top_level_offset
    }

    pub(crate) fn vertical_align(self, fields: &[&str]) -> Option<FormFieldVerticalAlign> {
        FormFieldVerticalAlign::from_raw_value(fields.get(self.vertical_align_slot())?)
    }

    pub(crate) const fn group_horizontal_align_slot(self) -> usize {
        53 + self.top_level_offset
    }

    pub(crate) fn group_horizontal_align(
        self,
        fields: &[&str],
    ) -> Option<FormFieldGroupHorizontalAlign> {
        FormFieldGroupHorizontalAlign::from_raw_value(
            fields.get(self.group_horizontal_align_slot())?,
        )
    }

    pub(crate) const fn group_vertical_align_slot(self) -> usize {
        54 + self.top_level_offset
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<FormFieldVerticalAlign> {
        FormFieldVerticalAlign::from_raw_value(fields.get(self.group_vertical_align_slot())?)
    }

    pub(crate) fn enabled(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.enabled_slot?)?.trim() == "0").then_some(false)
    }

    pub(crate) const fn warning_on_edit_representation_slot(self) -> usize {
        17 + self.top_level_offset
    }

    pub(crate) fn warning_on_edit_representation(
        self,
        fields: &[&str],
    ) -> Option<FormWarningOnEditRepresentation> {
        match fields
            .get(self.warning_on_edit_representation_slot())?
            .trim()
        {
            "0" => Some(FormWarningOnEditRepresentation::Show),
            "1" => Some(FormWarningOnEditRepresentation::DontShow),
            _ => None,
        }
    }

    pub(crate) const fn warning_on_edit_slot(self) -> usize {
        18 + self.top_level_offset
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

    pub(crate) fn spreadsheet_document_properties(
        self,
        fields: &[&str],
        options: &[&str],
    ) -> Option<FormSpreadsheetDocumentFieldProperties> {
        self.spreadsheet_document_options
            .then(|| FormSpreadsheetDocumentFieldProperties::from_raw_layout(fields, options))
            .flatten()
    }

    pub(crate) fn input_field_option<'a>(
        self,
        options: &'a [&'a str],
        slot: FormInputFieldExtendedOptionSlot,
    ) -> Option<&'a str> {
        self.input_field_options
            .then(|| options.get(slot.index()).copied())
            .flatten()
    }

    pub(crate) fn choice_button_picture(self, value: &[&str]) -> Option<FormPictureValueSchema> {
        self.input_field_options.then_some(())?;
        let picture = FormPictureValueSchema::from_raw_layout(value)?;
        matches!(
            picture.kind(),
            FormPictureValueKind::Empty | FormPictureValueKind::Reference
        )
        .then_some(picture)
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

    fn dimension(self, options: &[&str], slot: usize) -> Option<String> {
        let value = options.get(slot)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormButtonColorSchema;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormButtonCommonSchema {
    top_level_offset: usize,
}

impl FormButtonCommonSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
    ) -> Option<Self> {
        match (wrapper, field_count, item_tag, top_level_offset) {
            ("31", 52, "Button", 0) | ("31", 53, "Button", 1) => Some(Self { top_level_offset }),
            _ => None,
        }
    }

    pub(crate) fn enabled(self, fields: &[&str]) -> Option<bool> {
        (fields.get(7 + self.top_level_offset)?.trim() == "0").then_some(false)
    }

    pub(crate) const fn data_path_slot(self) -> Option<usize> {
        if self.top_level_offset == 0 {
            Some(9)
        } else {
            None
        }
    }

    pub(crate) fn height(self, fields: &[&str]) -> Option<String> {
        self.non_zero_dimension(fields, 17)
    }

    pub(crate) fn title_height(self, fields: &[&str]) -> Option<String> {
        self.non_zero_dimension(fields, 18)
    }

    pub(crate) fn font<'a>(self, fields: &'a [&'a str]) -> Option<&'a str> {
        let value = fields.get(22 + self.top_level_offset)?.trim();
        (value != "{7,3,0,1,100}").then_some(value)
    }

    pub(crate) fn horizontal_stretch(self, fields: &[&str]) -> Option<bool> {
        (fields.get(39 + self.top_level_offset)?.trim() == "1").then_some(true)
    }

    pub(crate) fn vertical_stretch(self, fields: &[&str]) -> Option<bool> {
        (fields.get(40 + self.top_level_offset)?.trim() == "1").then_some(true)
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(42 + self.top_level_offset)?.trim() {
            "0" => Some("Top"),
            "1" => Some("Center"),
            "2" => Some("Bottom"),
            "3" => None,
            _ => None,
        }
    }

    fn non_zero_dimension(self, fields: &[&str], slot: usize) -> Option<String> {
        let value = fields.get(slot + self.top_level_offset)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }
}

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
    const SHOW_IN_FOOTER_SLOT: usize = 21;
    const GROUP_HORIZONTAL_ALIGN_SLOT: usize = 53;
    const GROUP_VERTICAL_ALIGN_SLOT: usize = 54;
    const THREE_STATE_OPTION_SLOT: usize = 1;

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

    pub(crate) fn show_in_footer(self, fields: &[&str]) -> Option<bool> {
        match fields
            .get(Self::SHOW_IN_FOOTER_SLOT + self.top_level_offset)?
            .trim()
        {
            "0" => Some(false),
            "1" => None,
            _ => None,
        }
    }

    pub(crate) fn group_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields
            .get(Self::GROUP_HORIZONTAL_ALIGN_SLOT + self.top_level_offset)?
            .trim()
        {
            "0" => Some("Left"),
            "2" => Some("Right"),
            "3" => None,
            _ => None,
        }
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields
            .get(Self::GROUP_VERTICAL_ALIGN_SLOT + self.top_level_offset)?
            .trim()
        {
            "0" => Some("Top"),
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

    pub(crate) fn three_state(self, options: &[&str]) -> Option<bool> {
        match options
            .get(Self::THREE_STATE_OPTION_SLOT)
            .map(|field| field.trim())?
        {
            "1" => Some(true),
            "0" => None,
            _ => None,
        }
    }

    pub(crate) const fn three_state_option_slot(self) -> usize {
        Self::THREE_STATE_OPTION_SLOT
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

    pub(crate) const fn raw_slot_for_normalized(self, normalized_slot: usize) -> usize {
        if normalized_slot < self.prefix_slot {
            normalized_slot
        } else {
            normalized_slot + 1
        }
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
pub(crate) struct FormCommandBarSchema;

impl FormCommandBarSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;
    const CHILD_COUNT_SLOT: usize = 21;
    const ENABLED_SLOT: usize = 10;
    const WIDTH_SLOT: usize = 12;
    const HEIGHT_SLOT: usize = 13;
    const HORIZONTAL_STRETCH_SLOT: usize = 14;
    const GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET: usize = 3;
    const GROUP_VERTICAL_ALIGN_REVERSE_OFFSET: usize = 2;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        fields: &[&str],
        options: &[&str],
        source: &[&str],
    ) -> Option<Self> {
        if wrapper != "22"
            || item_tag != "CommandBar"
            || direct_discriminator != Some("0")
            || options.len() != 3
            || options.first().map(|field| field.trim()) != Some("1")
            || !matches!(
                options.get(1).map(|field| field.trim()),
                Some("0" | "1" | "2" | "3")
            )
        {
            return None;
        }

        let child_count = fields
            .get(Self::CHILD_COUNT_SLOT)?
            .trim()
            .parse::<usize>()
            .ok()?;
        let expected_field_count = child_count.checked_mul(2)?.checked_add(30)?;
        if fields.len() != expected_field_count
            || !matches!(
                fields.get(Self::ENABLED_SLOT).map(|field| field.trim()),
                Some("0" | "1")
            )
            || fields.get(Self::WIDTH_SLOT)?.trim().parse::<u32>().is_err()
            || fields
                .get(Self::HEIGHT_SLOT)?
                .trim()
                .parse::<u32>()
                .is_err()
            || !matches!(
                fields
                    .get(Self::HORIZONTAL_STRETCH_SLOT)
                    .map(|field| field.trim()),
                Some("0" | "1" | "2")
            )
            || !matches!(
                fields
                    .get(
                        fields
                            .len()
                            .checked_sub(Self::GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET)?
                    )
                    .map(|field| field.trim()),
                Some("2" | "3")
            )
            || !matches!(
                fields
                    .get(
                        fields
                            .len()
                            .checked_sub(Self::GROUP_VERTICAL_ALIGN_REVERSE_OFFSET)?
                    )
                    .map(|field| field.trim()),
                Some("1" | "2" | "3")
            )
        {
            return None;
        }

        let source_is_valid = match source {
            [source_id] => source_id.trim().parse::<i64>().is_ok(),
            [source_id, source_type] => {
                source_id.trim().parse::<i64>().is_ok()
                    && uuid::Uuid::parse_str(source_type.trim())
                        .ok()
                        .is_some_and(|value| !value.is_nil())
            }
            _ => false,
        };
        source_is_valid.then_some(Self)
    }

    pub(crate) fn enabled(self, fields: &[&str]) -> Option<bool> {
        (fields.get(Self::ENABLED_SLOT)?.trim() == "0").then_some(false)
    }

    pub(crate) fn width(self, fields: &[&str]) -> Option<String> {
        Self::dimension(fields, Self::WIDTH_SLOT)
    }

    pub(crate) fn height(self, fields: &[&str]) -> Option<String> {
        Self::dimension(fields, Self::HEIGHT_SLOT)
    }

    pub(crate) fn horizontal_stretch(self, fields: &[&str]) -> Option<bool> {
        match fields.get(Self::HORIZONTAL_STRETCH_SLOT)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }

    pub(crate) fn group_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields
            .get(
                fields
                    .len()
                    .checked_sub(Self::GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET)?,
            )?
            .trim()
        {
            "2" => Some("Right"),
            _ => None,
        }
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields
            .get(
                fields
                    .len()
                    .checked_sub(Self::GROUP_VERTICAL_ALIGN_REVERSE_OFFSET)?,
            )?
            .trim()
        {
            "1" => Some("Center"),
            "2" => Some("Bottom"),
            _ => None,
        }
    }

    fn dimension(fields: &[&str], slot: usize) -> Option<String> {
        let value = fields.get(slot)?.trim();
        value
            .parse::<u32>()
            .ok()
            .filter(|value| *value != 0)
            .map(|_| value.to_owned())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormChildItemShowTitleSchema {
    option_slot: usize,
    back_color_option_slot: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormSharedContainerContentChangeSchema {
    enable_content_change: Option<bool>,
}

impl FormSharedContainerContentChangeSchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        marker: Option<&str>,
    ) -> Option<Self> {
        if wrapper != "22" || field_count < 30 || (field_count - 30) % 2 != 0 {
            return None;
        }
        match (item_tag, direct_discriminator) {
            ("CommandBar", Some("0"))
            | ("Popup", Some("1"))
            | ("ColumnGroup", Some("2"))
            | ("Pages", Some("3"))
            | ("ButtonGroup", Some("6")) => {}
            _ => return None,
        }
        let enable_content_change = match marker {
            Some("0") => None,
            Some("1") => Some(true),
            _ => return None,
        };
        Some(Self {
            enable_content_change,
        })
    }

    pub(crate) const fn enable_content_change(self) -> Option<bool> {
        self.enable_content_change
    }

    pub(crate) fn supports_xml_tag(item_tag: &str) -> bool {
        matches!(
            item_tag,
            "CommandBar" | "Popup" | "ColumnGroup" | "Pages" | "ButtonGroup"
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormContainerReadOnlySchema;

impl FormContainerReadOnlySchema {
    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        if wrapper != "22" || field_count < 30 || (field_count - 30) % 2 != 0 {
            return None;
        }
        match (
            item_tag,
            direct_discriminator,
            options.len(),
            options.first().map(|field| field.trim()),
        ) {
            ("ColumnGroup", Some("2"), 12, Some("2")) | ("Page", Some("4"), 20, Some("18")) => {
                Some(Self)
            }
            _ => None,
        }
    }

    pub(crate) fn read_only(self, fields: &[&str]) -> Option<bool> {
        (fields.get(11).map(|field| field.trim()) == Some("1")).then_some(true)
    }
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

/// The form root's property bag: field 18 declares how many `key`/`value`
/// pairs follow at field 19, and the bag is that many pairs wide - a declared
/// count, not a fixed slot window.
///
/// UT 11.5.27.75 native tree, 5 075 attributable roots: the declared count is
/// 0 (2 881 roots), 1 (1 400), 3 (47), 4 (373), 5 (2), 6 (320) or 21 (52), and
/// in every single one of them the field is an integer, the layout carries the
/// full `19 + 2 * count` fields, and every key in the walk is an integer -
/// zero counter-examples. Reading the bag only when the count exceeds one
/// hides the 1 400 single-entry bags, 13 of which carry the
/// `UseForFoldersAndItems` entry under key 0.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootPropertyBagSchema {
    entry_count: usize,
}

impl FormRootPropertyBagSchema {
    pub(crate) const COUNT_SLOT: usize = 18;
    pub(crate) const FIRST_ENTRY_SLOT: usize = 19;

    pub(crate) fn from_raw_layout(count_field: Option<&str>, field_count: usize) -> Option<Self> {
        let entry_count = count_field?.trim().parse::<usize>().ok()?;
        let required = Self::FIRST_ENTRY_SLOT.checked_add(entry_count.checked_mul(2)?)?;
        (field_count >= required).then_some(Self { entry_count })
    }

    pub(crate) const fn entry_count(self) -> usize {
        self.entry_count
    }

    pub(crate) const fn key_slot(self, entry_index: usize) -> usize {
        Self::FIRST_ENTRY_SLOT + entry_index * 2
    }
}

/// `Customizable` sits alone in root field 14 of the `50` layout.
///
/// UT 11.5.27.75 native tree, 5 075 attributable roots: field 14 reads `0` for
/// all 359 roots whose native document carries `<Customizable>false</...>` and
/// `1` for all 4 716 that omit it - a total function with no counter-example.
/// Field 11 is the root's `Group` marker (`1` for the 39 roots that carry a
/// horizontal `Group`, `0` for the other 5 036), so pairing it into the
/// `Customizable` test only suppressed the 12 roots that have both.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootCustomizableSchema {
    slot: usize,
}

impl FormRootCustomizableSchema {
    const SLOT: usize = 14;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        field_count: usize,
    ) -> Option<Self> {
        (root_discriminator == Some("50") && field_count > Self::SLOT)
            .then_some(Self { slot: Self::SLOT })
    }

    pub(crate) fn customizable(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.slot)?.trim() {
            "0" => Some(false),
            _ => None,
        }
    }
}

/// `CustomSettingsFolder` is property-bag key 23: an `{"N", id}` reference to
/// one of the form's own items, with `0` standing for "no folder".
///
/// UT 11.5.27.75 native tree: 56 of the 5 075 attributable roots carry key 23.
/// It reads `{"N",0}` in the 42 whose native document omits the property, and a
/// non-zero item id in the 14 that carry it - and in all 14 the id resolves
/// through the form's own item table to exactly the native folder name. No
/// counter-example either way.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootCustomSettingsFolderSchema;

impl FormRootCustomSettingsFolderSchema {
    pub(crate) const PROPERTY_BAG_KEY: &'static str = "23";

    pub(crate) fn from_raw_layout(root_discriminator: Option<&str>) -> Option<Self> {
        (root_discriminator == Some("50")).then_some(Self)
    }

    pub(crate) fn item_id(self, value_fields: &[&str]) -> Option<String> {
        match (
            value_fields.first().map(|field| field.trim()),
            value_fields.get(1).map(|field| field.trim()),
            value_fields.len(),
        ) {
            (Some(r##""N""##), Some("0"), 2) => None,
            (Some(r##""N""##), Some(id), 2) if id.parse::<u64>().is_ok() => Some(id.to_owned()),
            _ => None,
        }
    }
}

/// `ConversationsRepresentation` lives in trailer slot 19 of the `50` layout,
/// not in the property bag.
///
/// UT 11.5.27.75 native tree, 5 075 attributable roots: trailer slot 19 reads
/// `2` for all 16 roots whose native document says `DontShow`, `1` for all 3
/// that say `Show`, and `0` for the remaining 5 056 that omit the property -
/// a total function with no counter-example. Property-bag key 21 carries the
/// report form's `AutoShowState` instead, which is why the previous bag-keyed
/// reader never emitted the property on any of the 5 075 roots.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootConversationsRepresentationSchema {
    trailer_slot: usize,
}

impl FormRootConversationsRepresentationSchema {
    const TRAILER_SLOT: usize = 19;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer_field_count: usize,
    ) -> Option<Self> {
        matches!((root_discriminator, trailer_field_count), (Some("50"), 24)).then_some(Self {
            trailer_slot: Self::TRAILER_SLOT,
        })
    }

    pub(crate) fn conversations_representation(self, trailer: &[&str]) -> Option<&'static str> {
        match trailer.get(self.trailer_slot)?.trim() {
            "1" => Some("Show"),
            "2" => Some("DontShow"),
            _ => None,
        }
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
pub(crate) enum FormRootVerticalAlign {
    Bottom,
}

impl FormRootVerticalAlign {
    pub(crate) fn from_raw_value(value: &str) -> Option<Self> {
        match value.trim() {
            "2" => Some(Self::Bottom),
            _ => None,
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "Bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    pub(crate) const fn raw_value(self) -> &'static str {
        match self {
            Self::Bottom => "2",
        }
    }

    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Bottom => "Bottom",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootVerticalAlignSchema {
    trailer_slot: usize,
}

impl FormRootVerticalAlignSchema {
    const TRAILER_SLOT: usize = 12;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer_field_count: usize,
    ) -> Option<Self> {
        matches!((root_discriminator, trailer_field_count), (Some("50"), 24)).then_some(Self {
            trailer_slot: Self::TRAILER_SLOT,
        })
    }

    pub(crate) fn vertical_align(self, trailer: &[&str]) -> Option<FormRootVerticalAlign> {
        FormRootVerticalAlign::from_raw_value(trailer.get(self.trailer_slot)?.trim())
    }

    pub(crate) const fn trailer_slot(self) -> usize {
        self.trailer_slot
    }

    pub(crate) fn accepts_raw_value(self, value: &str) -> bool {
        matches!(value.trim(), "2" | "3")
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
    Popup,
    ColumnGroup,
    Pages,
    Page,
    ButtonGroup,
    Table,
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
            "Popup" => Self::Popup,
            "ColumnGroup" => Self::ColumnGroup,
            "Pages" => Self::Pages,
            "Page" => Self::Page,
            "ButtonGroup" => Self::ButtonGroup,
            "Table" => Self::Table,
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
    ButtonGroupHeader,
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
    if wrapper == "22" && field_count >= 30 && (field_count - 30) % 2 == 0 {
        let admitted = matches!(
            (item_kind, direct_discriminator),
            (FormTooltipRepresentationItemKind::Popup, Some("1"))
                | (FormTooltipRepresentationItemKind::ColumnGroup, Some("2"))
                | (FormTooltipRepresentationItemKind::Pages, Some("3"))
                | (FormTooltipRepresentationItemKind::Page, Some("4"))
                | (FormTooltipRepresentationItemKind::ButtonGroup, Some("6"))
        );
        if admitted {
            return Some(FormTooltipRepresentationSchema {
                slot: field_count.checked_sub(7)?,
            });
        }
    }
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
        FormTooltipRepresentationItemKind::Popup | FormTooltipRepresentationItemKind::Pages => {
            Some(FormTooltipRepresentationXmlOrder::AfterTitle)
        }
        FormTooltipRepresentationItemKind::ButtonGroup => {
            Some(FormTooltipRepresentationXmlOrder::ButtonGroupHeader)
        }
        FormTooltipRepresentationItemKind::ColumnGroup => {
            Some(FormTooltipRepresentationXmlOrder::FieldProperties)
        }
        FormTooltipRepresentationItemKind::Page | FormTooltipRepresentationItemKind::Table => None,
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

pub(crate) fn form_tooltip_representation_supports_xml_tag(item_tag: &str) -> bool {
    !matches!(
        FormTooltipRepresentationItemKind::from_xml_tag(item_tag),
        FormTooltipRepresentationItemKind::Other
    )
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTooltipRepresentation {
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
    pub(crate) fn from_raw_scalar(value: &str) -> Option<Self> {
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

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "None" => Some(Self::None),
            "Balloon" => Some(Self::Balloon),
            "Button" => Some(Self::Button),
            "ShowAuto" => Some(Self::ShowAuto),
            "ShowTop" => Some(Self::ShowTop),
            "ShowLeft" => Some(Self::ShowLeft),
            "ShowBottom" => Some(Self::ShowBottom),
            "ShowRight" => Some(Self::ShowRight),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Omit => "0",
            Self::None => "1",
            Self::Balloon => "2",
            Self::Button => "3",
            Self::ShowAuto => "4",
            Self::ShowTop => "5",
            Self::ShowLeft => "6",
            Self::ShowBottom => "7",
            Self::ShowRight => "8",
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
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormInputFieldTailXmlProperty {
    MultipleValueDataPath,
    MultipleValuePresentDataPath,
    ListChoiceMode,
    ExtendedEditMultipleValues,
    AutoMarkIncomplete,
}

pub(crate) const FORM_INPUT_FIELD_TAIL_XML_ORDER: &[FormInputFieldTailXmlProperty] = &[
    FormInputFieldTailXmlProperty::ListChoiceMode,
    FormInputFieldTailXmlProperty::ExtendedEditMultipleValues,
    // The two multiple-value bound paths trail `ExtendedEditMultipleValues`,
    // `ChoiceButton` and `DataPath` and precede `ContextMenu`, `ExtendedTooltip`
    // and `Events`, and the value path precedes the presentation path, on all 3
    // native items that carry them.
    FormInputFieldTailXmlProperty::MultipleValueDataPath,
    FormInputFieldTailXmlProperty::MultipleValuePresentDataPath,
    FormInputFieldTailXmlProperty::AutoMarkIncomplete,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableXmlProperty {
    Representation,
    HeaderHeight,
    VerticalScrollBar,
    TitleLocation,
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
    MultipleChoice,
    RowInputMode,
    SelectionMode,
    RowSelectionMode,
    Header,
    Footer,
    HorizontalScrollBar,
    HorizontalLines,
    VerticalLines,
    UseAlternationRowColor,
    AutoInsertNewRow,
    AutoAddIncomplete,
    AutoMarkIncomplete,
    SearchOnInput,
    InitialListView,
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
    CurrentRowUse,
    ToolTip,
    ToolTipRepresentation,
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

pub(crate) fn normalize_form_table_command_bar_location_xml(value: &str) -> Option<&'static str> {
    match value {
        "None" => Some("None"),
        "Top" => Some("Top"),
        _ => None,
    }
}

pub(crate) fn encode_form_table_command_bar_location(value: &str) -> Option<&'static str> {
    match value {
        "None" => Some("0"),
        "Top" => Some("1"),
        _ => None,
    }
}

pub(crate) const FORM_TABLE_XML_ORDER: &[FormTableXmlProperty] = &[
    FormTableXmlProperty::Representation,
    // Native `Table` bodies run `Visible` -> `UserVisible` -> `TitleLocation`
    // (UT 11.5.27.75 native tree: Visible<UserVisible 2, Visible<TitleLocation 1,
    // UserVisible<TitleLocation 1, no counter-example).
    FormTableXmlProperty::Visible,
    FormTableXmlProperty::UserVisible,
    FormTableXmlProperty::TitleLocation,
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
    FormTableXmlProperty::MultipleChoice,
    FormTableXmlProperty::RowInputMode,
    FormTableXmlProperty::SelectionMode,
    FormTableXmlProperty::RowSelectionMode,
    FormTableXmlProperty::Header,
    FormTableXmlProperty::HorizontalScrollBar,
    // `VerticalScrollBar` trails `HorizontalScrollBar` (4), `Header` (13),
    // `ChangeRowOrder` (7), `SkipOnInput` (3), `SelectionMode` (2) and
    // `ChoiceMode` (2) and precedes `HorizontalLines` (7),
    // `UseAlternationRowColor` and `AutoInsertNewRow`, with no counter-example.
    FormTableXmlProperty::VerticalScrollBar,
    FormTableXmlProperty::HorizontalLines,
    FormTableXmlProperty::VerticalLines,
    // `HeaderHeight` trails `Representation`, `CommandBarLocation`, `ReadOnly`,
    // `SkipOnInput`, `DefaultItem`, `ChangeRowSet`, `ChangeRowOrder`, `Width`,
    // `Height`, `HeightInTableRows`, `SelectionMode`, `RowSelectionMode` and
    // `Header`, and precedes `UseAlternationRowColor`, `AutoInsertNewRow`,
    // `EnableStartDrag`, `FileDragMode`, `DataPath`, `Title` and `CommandSet`
    // on all 32 native occurrences, with no counter-example.
    FormTableXmlProperty::HeaderHeight,
    // `Footer` trails `Representation` (31), `SkipOnInput` (13),
    // `ChangeRowOrder` (8), `ChangeRowSet` (8), `ReadOnly` (4),
    // `TitleLocation` (4), `CommandBarLocation` (3), `AutoMaxHeight` (2),
    // `HeightInTableRows` (2), `AutoMaxWidth` (1), `DefaultItem` (1),
    // `HeaderHeight` (1) and `Height` (1), and precedes `DataPath` (39),
    // `Events` (31), `FileDragMode` (29), `Title` (27), `EnableDrag` (23),
    // `EnableStartDrag` (23), `AutoInsertNewRow` (22), `CommandSet` (9),
    // `AutoAddIncomplete` (7), the three search/status locations (4 each),
    // `VerticalStretch` (3) and `UseAlternationRowColor` (2), across all 39
    // native occurrences with no pair counted both ways.
    FormTableXmlProperty::Footer,
    FormTableXmlProperty::UseAlternationRowColor,
    FormTableXmlProperty::AutoInsertNewRow,
    FormTableXmlProperty::AutoAddIncomplete,
    FormTableXmlProperty::AutoMarkIncomplete,
    FormTableXmlProperty::SearchOnInput,
    FormTableXmlProperty::InitialListView,
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
    FormTableXmlProperty::ToolTip,
    FormTableXmlProperty::ToolTipRepresentation,
    FormTableXmlProperty::SearchStringLocation,
    FormTableXmlProperty::ViewStatusLocation,
    FormTableXmlProperty::SearchControlLocation,
    FormTableXmlProperty::CurrentRowUse,
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
    Footer,
    HorizontalScrollBar,
    HorizontalLines,
    VerticalLines,
    UseAlternationRowColor,
    AutoInsertNewRow,
    InitialListView,
    InitialTreeView,
    EnableStartDrag,
    EnableDrag,
}

impl FormTableSlot {
    const ALL: [Self; 21] = [
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
        Self::HorizontalScrollBar,
        Self::HorizontalLines,
        Self::VerticalLines,
        Self::UseAlternationRowColor,
        Self::AutoInsertNewRow,
        Self::InitialListView,
        Self::InitialTreeView,
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
            // The column footer's own switch, one slot past the header's, and
            // the only slot that tells the two apart: `1` on all 39 native
            // tables that write `<Footer>true</Footer>` and `0` on all 4 490
            // that write nothing, with no counter-example.
            Self::Footer => 28,
            Self::HorizontalScrollBar => 30,
            Self::HorizontalLines => 32,
            Self::VerticalLines => 33,
            Self::UseAlternationRowColor => 36,
            Self::AutoInsertNewRow => 37,
            Self::InitialListView => 38,
            Self::InitialTreeView => 39,
            Self::EnableStartDrag => 52,
            Self::EnableDrag => 53,
        }
    }

    fn accepts(self, field: &str) -> bool {
        match self {
            Self::RowInputMode => matches!(field.trim(), "0" | "2"),
            Self::HorizontalScrollBar => matches!(field.trim(), "0" | "1" | "2"),
            Self::InitialListView | Self::InitialTreeView => {
                matches!(field.trim(), "0" | "1" | "2")
            }
            Self::Width | Self::Height => field.trim().parse::<u32>().is_ok(),
            _ => matches!(field.trim(), "0" | "1"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableHorizontalScrollBar {
    DontUse,
    UseAlways,
}

impl FormTableHorizontalScrollBar {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::DontUse => "DontUse",
            Self::UseAlways => "UseAlways",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "DontUse" => Some(Self::DontUse),
            "UseAlways" => Some(Self::UseAlways),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::DontUse => "0",
            Self::UseAlways => "1",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableRowPictureDataPath<'a> {
    Empty,
    Payload(&'a str),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableCurrentRowUse {
    Choice,
    SelectionPresentation,
    SelectionPresentationAndChoice,
}

impl FormTableCurrentRowUse {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Choice => "Choice",
            Self::SelectionPresentation => "SelectionPresentation",
            Self::SelectionPresentationAndChoice => "SelectionPresentationAndChoice",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "Choice" => Some(Self::Choice),
            "SelectionPresentation" => Some(Self::SelectionPresentation),
            "SelectionPresentationAndChoice" => Some(Self::SelectionPresentationAndChoice),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Choice => "1",
            Self::SelectionPresentation => "2",
            Self::SelectionPresentationAndChoice => "3",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableSearchOnInput {
    Use,
    DontUse,
}

impl FormTableSearchOnInput {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Use => "Use",
            Self::DontUse => "DontUse",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "Use" => Some(Self::Use),
            "DontUse" => Some(Self::DontUse),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Use => "0",
            Self::DontUse => "1",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableInitialListView {
    Beginning,
    End,
}

impl FormTableInitialListView {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::Beginning => "Beginning",
            Self::End => "End",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "Beginning" => Some(Self::Beginning),
            "End" => Some(Self::End),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::Beginning => "0",
            Self::End => "1",
        }
    }
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
enum FormTableFileDragMode {
    AsFile,
    Omit,
}

impl FormTableFileDragMode {
    fn from_raw(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::AsFile),
            "1" => Some(Self::Omit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FormTableSkipOnInput {
    False,
    True,
    Omit,
}

impl FormTableSkipOnInput {
    fn from_raw(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::False),
            "1" => Some(Self::True),
            "2" => Some(Self::Omit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormTableSchema;

impl FormTableSchema {
    const BASE_FIELD_COUNT: usize = 99;
    const COUNTED_PROPERTY_BAG_PAIR_COUNT_SLOT: usize = 54;
    const DATA_PATH_SLOT: usize = 11;
    const ROW_PICTURE_DATA_PATH_SLOT: usize = 43;
    const ROWS_PICTURE_SLOT: usize = 44;
    const BACK_COLOR_SLOT: usize = 45;
    const TEXT_COLOR_SLOT: usize = 46;
    const BORDER_COLOR_SLOT: usize = 47;
    const FILE_DRAG_MODE_REVERSE_OFFSET: usize = 2;
    const MULTIPLE_CHOICE_REVERSE_OFFSET: usize = 34;
    const SKIP_ON_INPUT_REVERSE_OFFSET: usize = 30;
    const SEARCH_ON_INPUT_REVERSE_OFFSET: usize = 29;
    const SEARCH_STRING_LOCATION_REVERSE_OFFSET: usize = 25;
    const VIEW_STATUS_LOCATION_REVERSE_OFFSET: usize = 24;
    const SEARCH_CONTROL_LOCATION_REVERSE_OFFSET: usize = 23;
    const TOOLTIP_REPRESENTATION_REVERSE_OFFSET: usize = 28;
    const CURRENT_ROW_USE_REVERSE_OFFSET: usize = 5;
    // This scalar is part of the fixed tail, not the counted property bag.
    // Native ibcmd emits AutoMaxWidth=false only for raw code 0 here.
    const AUTO_MAX_WIDTH_REVERSE_OFFSET: usize = 15;
    // Fixed tail scalar: 0=false, 1=true, 2=platform default (omitted).
    const AUTO_ADD_INCOMPLETE_REVERSE_OFFSET: usize = 36;

    pub(crate) fn from_raw_layout(wrapper: &str, item_tag: &str, fields: &[&str]) -> Option<Self> {
        if wrapper != "55"
            || item_tag != "Table"
            || fields.first().map(|field| field.trim()) != Some("55")
            || fields.len() < Self::BASE_FIELD_COUNT
        {
            return None;
        }
        // The suffix combines several paired sections; the bag at slot 54 is only one of them.
        if (fields.len() - Self::BASE_FIELD_COUNT) % 2 != 0 {
            return None;
        }

        if !FormTableSlot::ALL.iter().all(|slot| {
            fields
                .get(slot.index())
                .is_some_and(|field| slot.accepts(field))
        }) {
            return None;
        }
        FormTableFileDragMode::from_raw(Self::reverse_field(
            fields,
            Self::FILE_DRAG_MODE_REVERSE_OFFSET,
        )?)?;
        if !matches!(
            Self::reverse_field(fields, Self::MULTIPLE_CHOICE_REVERSE_OFFSET)?.trim(),
            "0" | "1"
        ) {
            return None;
        }
        FormTableSkipOnInput::from_raw(Self::reverse_field(
            fields,
            Self::SKIP_ON_INPUT_REVERSE_OFFSET,
        )?)?;
        if !matches!(
            Self::reverse_field(fields, Self::SEARCH_ON_INPUT_REVERSE_OFFSET)?.trim(),
            "0" | "1" | "2"
        ) {
            return None;
        }
        if !matches!(
            Self::reverse_field(fields, Self::CURRENT_ROW_USE_REVERSE_OFFSET)?.trim(),
            "0" | "1" | "2" | "3"
        ) {
            return None;
        }
        if !matches!(
            Self::reverse_field(fields, Self::AUTO_MAX_WIDTH_REVERSE_OFFSET)?.trim(),
            "0" | "1"
        ) {
            return None;
        }
        if !matches!(
            Self::reverse_field(fields, Self::AUTO_ADD_INCOMPLETE_REVERSE_OFFSET)?.trim(),
            "0" | "1" | "2"
        ) {
            return None;
        }
        Some(Self)
    }

    pub(crate) const fn counted_property_bag_pair_count_slot(self) -> usize {
        Self::COUNTED_PROPERTY_BAG_PAIR_COUNT_SLOT
    }

    pub(crate) fn counted_property_bag_bounds(self, fields: &[&str]) -> Option<(usize, usize)> {
        let pair_count = fields
            .get(Self::COUNTED_PROPERTY_BAG_PAIR_COUNT_SLOT)?
            .trim()
            .parse::<usize>()
            .ok()?;
        let start = Self::COUNTED_PROPERTY_BAG_PAIR_COUNT_SLOT.checked_add(1)?;
        let end = pair_count.checked_mul(2)?.checked_add(start)?;
        (end <= fields.len()).then_some((start, end))
    }

    pub(crate) fn counted_property_bag_value_slot(
        self,
        fields: &[&str],
        expected_key: FormTablePropertyBagKey,
    ) -> Option<usize> {
        let (start, end) = self.counted_property_bag_bounds(fields)?;
        let mut result = None;
        for key_slot in (start..end).step_by(2) {
            let raw_key = fields.get(key_slot)?.trim();
            let key = raw_key.parse::<usize>().ok()?;
            if key.to_string() != raw_key
                || (start..key_slot)
                    .step_by(2)
                    .any(|previous| fields[previous].trim() == raw_key)
            {
                return None;
            }
            if raw_key == expected_key.key() {
                result = Some(key_slot + 1);
            }
        }
        result
    }

    pub(crate) const fn tooltip_slot(self) -> usize {
        10
    }

    pub(crate) fn tooltip_representation_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::TOOLTIP_REPRESENTATION_REVERSE_OFFSET)?;
        FormTooltipRepresentation::from_raw_scalar(fields.get(slot)?.trim())?;
        Some(slot)
    }

    pub(crate) fn title_location(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(6)?.trim() {
            "1" => Some("Auto"),
            "3" => Some("Top"),
            _ => None,
        }
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

    pub(crate) fn current_row_use_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::CURRENT_ROW_USE_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "1" | "2" | "3").then_some(slot)
    }

    pub(crate) fn current_row_use(self, fields: &[&str]) -> Option<FormTableCurrentRowUse> {
        match fields.get(self.current_row_use_slot(fields)?)?.trim() {
            "1" => Some(FormTableCurrentRowUse::Choice),
            "2" => Some(FormTableCurrentRowUse::SelectionPresentation),
            "3" => Some(FormTableCurrentRowUse::SelectionPresentationAndChoice),
            _ => None,
        }
    }

    pub(crate) fn auto_max_width_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::AUTO_MAX_WIDTH_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "1").then_some(slot)
    }

    pub(crate) fn auto_max_width(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.auto_max_width_slot(fields)?)?.trim() == "0").then_some(false)
    }

    pub(crate) fn auto_add_incomplete_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::AUTO_ADD_INCOMPLETE_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "1" | "2").then_some(slot)
    }

    pub(crate) fn auto_add_incomplete(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.auto_add_incomplete_slot(fields)?)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            "2" => None,
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

    pub(crate) fn multiple_choice_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::MULTIPLE_CHOICE_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "1").then_some(slot)
    }

    pub(crate) fn multiple_choice(self, fields: &[&str]) -> Option<bool> {
        (fields.get(self.multiple_choice_slot(fields)?)?.trim() == "1").then_some(true)
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

    pub(crate) fn footer(self, fields: &[&str]) -> Option<bool> {
        (fields.get(FormTableSlot::Footer.index())?.trim() == "1").then_some(true)
    }

    pub(crate) const fn horizontal_scroll_bar_slot(self) -> usize {
        FormTableSlot::HorizontalScrollBar.index()
    }

    pub(crate) fn horizontal_scroll_bar(
        self,
        fields: &[&str],
    ) -> Option<FormTableHorizontalScrollBar> {
        match fields.get(self.horizontal_scroll_bar_slot())?.trim() {
            "0" => Some(FormTableHorizontalScrollBar::DontUse),
            "1" => Some(FormTableHorizontalScrollBar::UseAlways),
            _ => None,
        }
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

    pub(crate) fn file_drag_mode_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::FILE_DRAG_MODE_REVERSE_OFFSET)?;
        FormTableFileDragMode::from_raw(fields.get(slot)?)?;
        Some(slot)
    }

    pub(crate) fn file_drag_mode_raw_code(self, value: &str) -> Option<&'static str> {
        match value {
            "AsFile" => Some("0"),
            _ => None,
        }
    }

    pub(crate) fn file_drag_mode(self, fields: &[&str]) -> Option<&'static str> {
        match FormTableFileDragMode::from_raw(fields.get(self.file_drag_mode_slot(fields)?)?)? {
            FormTableFileDragMode::AsFile => Some("AsFile"),
            FormTableFileDragMode::Omit => None,
        }
    }

    pub(crate) fn skip_on_input(self, fields: &[&str]) -> Option<bool> {
        match FormTableSkipOnInput::from_raw(Self::reverse_field(
            fields,
            Self::SKIP_ON_INPUT_REVERSE_OFFSET,
        )?)? {
            FormTableSkipOnInput::False => Some(false),
            FormTableSkipOnInput::True => Some(true),
            FormTableSkipOnInput::Omit => None,
        }
    }

    pub(crate) fn search_on_input_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::SEARCH_ON_INPUT_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "1" | "2").then_some(slot)
    }

    pub(crate) fn search_on_input(self, fields: &[&str]) -> Option<FormTableSearchOnInput> {
        match fields.get(self.search_on_input_slot(fields)?)?.trim() {
            "0" => Some(FormTableSearchOnInput::Use),
            "1" => Some(FormTableSearchOnInput::DontUse),
            _ => None,
        }
    }

    pub(crate) fn initial_list_view_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = FormTableSlot::InitialListView.index();
        matches!(fields.get(slot)?.trim(), "0" | "1" | "2").then_some(slot)
    }

    pub(crate) fn initial_list_view(self, fields: &[&str]) -> Option<FormTableInitialListView> {
        match fields.get(self.initial_list_view_slot(fields)?)?.trim() {
            "0" => Some(FormTableInitialListView::Beginning),
            "1" => Some(FormTableInitialListView::End),
            _ => None,
        }
    }

    pub(crate) fn initial_tree_view_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = FormTableSlot::InitialTreeView.index();
        matches!(fields.get(slot)?.trim(), "0" | "1" | "2").then_some(slot)
    }

    pub(crate) fn initial_tree_view(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(self.initial_tree_view_slot(fields)?)?.trim() {
            "1" => Some("ExpandTopLevel"),
            "2" => Some("ExpandAllLevels"),
            _ => None,
        }
    }

    fn reverse_field<'a>(fields: &[&'a str], reverse_offset: usize) -> Option<&'a str> {
        fields
            .len()
            .checked_sub(reverse_offset)
            .and_then(|slot| fields.get(slot))
            .copied()
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

impl FormSpreadsheetDocumentFieldProperties {
    fn from_raw_layout(fields: &[&str], options: &[&str]) -> Option<Self> {
        if fields.len() != 59
            || fields.get(5).map(|field| field.trim()) != Some("6")
            || options.len() != 32
            || options.first().map(|field| field.trim()) != Some("13")
        {
            return None;
        }

        let option = |slot: usize| options.get(slot).map(|field| field.trim());
        let dimension = |slot: usize, default: &str| {
            option(slot)
                .filter(|value| *value != "0" && *value != default)
                .filter(|value| value.parse::<u32>().is_ok())
                .map(str::to_owned)
        };
        let explicit_true = |slot: usize| (option(slot) == Some("1")).then_some(true);
        let explicit_false = |slot: usize| (option(slot) == Some("0")).then_some(false);
        let scroll_bar = |slot: usize| match option(slot) {
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => None,
        };

        Some(Self {
            default_item: (fields.get(16)?.trim() == "1").then_some(true),
            width: dimension(1, "50"),
            height: dimension(2, "10"),
            auto_max_width: explicit_false(20),
            auto_max_height: explicit_false(23),
            vertical_stretch: explicit_false(4),
            show_grid: explicit_true(5),
            show_headers: explicit_true(6),
            show_cell_names: explicit_true(25),
            show_row_and_column_names: explicit_true(26),
            vertical_scroll_bar: scroll_bar(28),
            horizontal_scroll_bar: scroll_bar(29),
            edit: explicit_true(13),
            // Slot 30 is one code, not a two-value flag with a hole in it: of
            // the 222 `SpreadSheetDocumentField` option tuples UT 11.5.27.75
            // spells out, 190 hold `1` and write nothing, 15 hold `0` and write
            // `WhenActive`, 1 holds `3` and writes
            // `WhenMultipleCellsSelected`, and the remaining 16 hold `2` --
            // which are, item for item, exactly the 16 items the platform
            // writes `<SelectionShowMode>DontShow</SelectionShowMode>` on, with
            // no miss and no item written where the platform writes none.
            selection_show_mode: match option(30) {
                Some("0") => Some("WhenActive"),
                Some("2") => Some("DontShow"),
                Some("3") => Some("WhenMultipleCellsSelected"),
                _ => None,
            },
            output: (option(12) == Some("1")).then_some("Enable"),
            protection: explicit_true(10),
            enable_start_drag: explicit_false(16),
            enable_drag: explicit_false(17),
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableRootPropertyBagKey {
    RestoreCurrentRow,
    TopLevelParent,
    ShowRoot,
    AllowRootChoice,
}

impl FormTableRootPropertyBagKey {
    pub(crate) const fn key(self) -> usize {
        match self {
            Self::RestoreCurrentRow => 9,
            Self::TopLevelParent => 10,
            Self::ShowRoot => 11,
            Self::AllowRootChoice => 12,
        }
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
    ChoiceList,
    Width,
    Height,
    HorizontalStretch,
    VerticalStretch,
    Wrap,
    PasswordMode,
    MultiLine,
    ExtendedEdit,
    MarkNegatives,
    ChoiceListButton,
    ChoiceButton,
    ClearButton,
    SpinButton,
    OpenButton,
    MinValue,
    MaxValue,
    Mask,
    ListChoiceMode,
    ChoiceButtonPicture,
    ChoiceListHeight,
    DropListWidth,
    QuickChoice,
    AutoCellHeight,
    ChoiceFoldersAndItems,
    ChoiceParameterLinks,
    ChoiceParameters,
    AutoChoiceIncomplete,
    AutoMarkIncomplete,
    ChooseType,
    IncompleteChoiceMode,
    Format,
    EditFormat,
    Font,
    TextEdit,
    TypeLink,
    EditTextUpdate,
    CreateButton,
    ChoiceButtonRepresentation,
    DropListButton,
    ChoiceHistoryOnInput,
    AutoMaxWidth,
    MaxWidth,
    AutoMaxHeight,
    MaxHeight,
    TypeDomainEnabled,
    HeightControlVariant,
    ChoiceParameterLinksDuplicate,
    ExtendedEditMultipleValues,
    AutoShowOpenButtonMode,
}

impl FormInputFieldExtendedOptionSlot {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::ChoiceList => 1,
            Self::Width => 2,
            Self::Height => 3,
            Self::HorizontalStretch => 4,
            Self::VerticalStretch => 5,
            Self::Wrap => 6,
            Self::PasswordMode => 7,
            Self::MultiLine => 8,
            Self::ExtendedEdit => 9,
            // `1` writes `<MarkNegatives>true</MarkNegatives>` and `2` writes
            // nothing, on every one of the 46 850 `InputField` items of the
            // attributable native forms with no counter-example.
            Self::MarkNegatives => 10,
            Self::ChoiceListButton => 11,
            Self::ChoiceButton => 12,
            Self::ClearButton => 13,
            Self::SpinButton => 14,
            Self::OpenButton => 15,
            Self::MinValue => 16,
            Self::MaxValue => 17,
            Self::Mask => 18,
            Self::ListChoiceMode => 19,
            Self::ChoiceButtonPicture => 20,
            // A zero writes nothing; any other value is written verbatim. Holds on
            // all 46 850 attributable `InputField` items with no counter-example.
            Self::ChoiceListHeight => 21,
            Self::DropListWidth => 22,
            Self::QuickChoice => 23,
            Self::ChoiceFoldersAndItems => 24,
            Self::ChoiceParameterLinks => 26,
            Self::ChoiceParameters => 27,
            Self::AutoCellHeight => 28,
            Self::AutoChoiceIncomplete => 28,
            Self::Format => 29,
            Self::EditFormat => 30,
            Self::AutoMarkIncomplete => 31,
            Self::ChooseType => 32,
            Self::IncompleteChoiceMode => 33,
            Self::Font => 40,
            Self::TextEdit => 41,
            Self::TypeLink => 42,
            Self::EditTextUpdate => 43,
            Self::CreateButton => 45,
            Self::ChoiceButtonRepresentation => 46,
            Self::DropListButton => 47,
            Self::ChoiceHistoryOnInput => 48,
            Self::AutoMaxWidth => 49,
            Self::MaxWidth => 50,
            Self::AutoMaxHeight => 52,
            Self::MaxHeight => 53,
            // `0` writes `<TypeDomainEnabled>false</TypeDomainEnabled>`, `1`
            // writes nothing; no other code occurs on the attributable items.
            Self::TypeDomainEnabled => 35,
            // `2 -> UseContentHeight`, `1 -> UseHeightInFormRows`, `0 -> nothing`.
            Self::HeightControlVariant => 54,
            Self::ChoiceParameterLinksDuplicate => 64,
            Self::ExtendedEditMultipleValues => 65,
            // `1 -> Always`, `2 -> FilledOnly`, `0` writes nothing. Of the
            // 49 951 `InputField` option tuples the UT 11.5.27.75 form bodies
            // spell out, 49 936 hold `0` here and none of their items carries
            // an `<AutoShowOpenButtonMode>`; the 13 that hold `2` and the 2
            // that hold `1` are, item for item, exactly the eight items the
            // platform writes `FilledOnly` on and the two it writes `Always`
            // on -- no other code occurs and there is no miss on either side.
            Self::AutoShowOpenButtonMode => 56,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFieldTopLevelSlot {
    DefaultItem,
    TitleTextColor,
    TitleFont,
    FooterTextColor,
    FooterFont,
}

impl FormFieldTopLevelSlot {
    pub(crate) const fn index(self, top_level_offset: usize) -> usize {
        match self {
            Self::DefaultItem => 16 + top_level_offset,
            // The title colour is the slot immediately ahead of the title font,
            // exactly as it is for the grouping controls (16 before 17) and for
            // `Table` (49 before 50). On the 90 000-odd field items of the
            // native UT 11.5.27.75 form dumps this slot holds the unset colour
            // on every item without a `<TitleTextColor>` and a readable colour
            // on every one of the 180 that carry it.
            Self::TitleTextColor => 31 + top_level_offset,
            Self::TitleFont => 32 + top_level_offset,
            // The footer's own colour and font sit two and four slots past the
            // title's. The colour slot holds the unset tuple on every native
            // field item without a `<FooterTextColor>` and a readable style
            // reference on all 6 that carry one; the font slot holds the empty
            // `AutoFont` default on every item without a `<FooterFont>` and a
            // font tuple on all 43 that carry one.
            Self::FooterTextColor => 34 + top_level_offset,
            Self::FooterFont => 36 + top_level_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormLabelFieldOptionSlot {
    Width,
    Height,
    HorizontalStretch,
    VerticalStretch,
    MarkNegatives,
    Format,
    Hiperlink,
    TextColor,
    Font,
    AutoMaxWidth,
    MaxWidth,
    AutoMaxHeight,
    MaxHeight,
}

impl FormLabelFieldOptionSlot {
    /// Slots of the 20-member `11`-discriminated `LabelField` option tuple.
    ///
    /// The geometry slots are the ones that reproduce every `<Width>`,
    /// `<Height>`, `<MaxWidth>`, `<MaxHeight>`, `<AutoMaxWidth>`,
    /// `<AutoMaxHeight>`, `<HorizontalStretch>` and `<VerticalStretch>` the
    /// platform writes on the 8 337 `LabelField` items of the native
    /// "1С:Управление торговлей 11.5.27.75" form dumps, with no misses and no
    /// false positives on the items that carry none of them.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Width => 1,
            Self::Height => 2,
            Self::HorizontalStretch => 3,
            Self::VerticalStretch => 4,
            // `1` writes `<MarkNegatives>true</MarkNegatives>` and `2` writes
            // nothing, on every one of the 28 558 `LabelField` items of the
            // attributable native forms with no counter-example.
            Self::MarkNegatives => 5,
            Self::Format => 6,
            Self::Hiperlink => 7,
            Self::TextColor => 8,
            Self::Font => 10,
            Self::AutoMaxWidth => 15,
            Self::MaxWidth => 16,
            Self::AutoMaxHeight => 18,
            Self::MaxHeight => 19,
        }
    }
}
