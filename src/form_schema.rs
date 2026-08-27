/// Placeholder for a trailing member a short record or property-bag revision
/// does not carry.  Deliberately unparseable as a 1C scalar, quoted string or
/// block, so a reader that reaches one refuses (doctrine point 2) instead of
/// reading a fabricated default (doctrine point 6).
///
/// It lives here rather than beside the normalizer that writes it because the
/// schema has to recognize it too: a slot addressed by a schema that the short
/// revision does not reach is *absent*, which is a different answer from
/// *malformed*.
pub(crate) const FORM_ABSENT_MEMBER: &str = "\u{1}absent";

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

// Platform type IDs used by serialized Form column patterns. Every prefix below
// - `dcscor`, `dcsset` and `v8` - is declared on the root element of every
// `Form.xml` the platform writes, so these references are emitted bare, with no
// namespace attribute of their own, exactly as the platform writes them.
//
// The whole table is scoped to a Form attribute column because that is the only
// role in which these identifiers occur: across the 1 245 files that still
// differ from the native UT 11.5.27.75 tree, the seven references below appear
// on 13 native-only lines in 6 files and every one of those lines is inside a
// `<Column>` of a Form attribute. None of the seven appears on our side at all,
// in any role, so resolving them can only add lines that the platform writes.
const FORM_COLUMN_BUILTIN_TYPE_REFERENCES: &[(&str, &str)] = &[
    ("f6841c6b-6c71-4c82-ae9e-d08b49db326c", "dcsset:Filter"),
    (
        "dcbf2698-3c1f-4a22-997f-48070ae9bd64",
        "dcsset:DataCompositionComparisonType",
    ),
    (
        "a090004e-b706-453f-aa10-090a77b53757",
        "dcsset:DataCompositionFieldPlacement",
    ),
    (
        "af4a19b5-da3d-406f-be0c-81143e400452",
        "dcscor:DataCompositionSortDirection",
    ),
    (
        "0e0850cf-0634-414e-85ba-9a88a8bd44c4",
        "dcscor:DataCompositionGroupType",
    ),
    (
        "c6a52555-d20f-452c-bfc2-1b53e9a56063",
        "dcscor:DataCompositionPeriodAdditionType",
    ),
    ("913e8016-6e90-47a0-b2a0-4513f4edad61", "dcscor:Field"),
    ("98ea8e5a-b586-442b-b944-6e3447734aa7", "v8:FillChecking"),
];

pub(crate) fn form_attribute_column_builtin_type_reference(type_id: &str) -> Option<&'static str> {
    FORM_COLUMN_BUILTIN_TYPE_REFERENCES
        .iter()
        .find_map(|(candidate, reference)| {
            type_id
                .eq_ignore_ascii_case(candidate)
                .then_some(*reference)
        })
}

// A `ChoiceList` item whose value is a design-time value of a platform-defined
// type names the type by identifier and the member by ordinal, and the platform
// writes both out: the type's QName as the `xsi:type` of the `<Value>` element
// and the member's own spelling as its text.  Neither spelling is derivable
// from the bytes, so this is the whole of what the corpus proves, member for
// member, and an ordinal that is not listed is a refusal.
//
// The prefixes are declared on the root element of every `Form.xml` the
// platform writes -- `dcsset` and `ent` both sit in the fixed prologue -- so
// the references are emitted bare, exactly as the platform writes them, just
// like the attribute-column table above.  `dcbf2698` occurs in both roles and
// is spelled the same way in each; the test below pins that agreement.
//
// Evidence, item for item and in order:
//
// * ERP УХ 3.2.12.6 `Catalogs/Запросы/Forms/НастройкаОтборов` spells the six
//   items `0,1,7,11,9,8` of `dcbf2698` and the platform writes `Equal`,
//   `NotEqual`, `InList`, `NotInList`, `InHierarchy`, `InListByHierarchy`;
//   `Catalogs/ПоложениеОЗакупках/Forms/ФормаЭлемента` spells `0,1,7,11,14,15`
//   and the platform writes `Equal`, `NotEqual`, `InList`, `NotInList`,
//   `Filled`, `NotFilled`.  Those twelve items are every
//   `dcsset:DataCompositionComparisonType` value in the configuration, and the
//   eight ordinals are every ordinal it spells.
// * `ChartsOfAccounts/Хозрасчетный/Forms/ФормаСчета` spells `0,1,2` of
//   `872f7198` and the platform writes `Active`, `Passive`, `ActivePassive`;
//   БСП демо 3.1.12.297 `ChartsOfAccounts/_ДемоОсновной/Forms/ФормаСчета`
//   spells the same three ordinals and the platform writes the same three
//   members.  No other configuration of the eight writes a design-time
//   platform value at all.
const FORM_DESIGN_TIME_PLATFORM_VALUE_TYPES: &[(&str, &str, &[(&str, &str)])] = &[
    (
        "dcbf2698-3c1f-4a22-997f-48070ae9bd64",
        "dcsset:DataCompositionComparisonType",
        &[
            ("0", "Equal"),
            ("1", "NotEqual"),
            ("7", "InList"),
            ("8", "InListByHierarchy"),
            ("9", "InHierarchy"),
            ("11", "NotInList"),
            ("14", "Filled"),
            ("15", "NotFilled"),
        ],
    ),
    (
        "872f7198-7083-4e3e-b57e-a2a9802c769e",
        "ent:AccountType",
        &[("0", "Active"), ("1", "Passive"), ("2", "ActivePassive")],
    ),
];

/// The QName and member spelling the platform writes for one design-time value
/// of a platform-defined type, or `None` when the corpus has never shown what
/// this type or this ordinal is spelled as.
pub(crate) fn form_choice_list_design_time_platform_value(
    type_id: &str,
    member_ordinal: &str,
) -> Option<(&'static str, &'static str)> {
    FORM_DESIGN_TIME_PLATFORM_VALUE_TYPES
        .iter()
        .find(|(candidate, _, _)| type_id.eq_ignore_ascii_case(candidate))
        .and_then(|(_, reference, members)| {
            members
                .iter()
                .find_map(|(ordinal, member)| (*ordinal == member_ordinal).then_some(*member))
                .map(|member| (*reference, member))
        })
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
    /// A single negative marker: the group is bound to a *standard* member of
    /// the attribute's own family, not to a column the attribute declares.
    /// Declared column ids are non-negative, so the two never collide.
    StandardMember,
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
            // A group bound straight to the attribute carries columns just like
            // every other binding kind; the column count was additionally
            // pinned to zero, which refused the group whole and dropped it
            // silently. Perepis of the eight stand corpora over every
            // `<AdditionalColumns table="…">` whose table names an attribute
            // and not one of its columns: 14 are written self-closed (no
            // column) and 34 carry columns. The refusal cost 21 `uh` form
            // bodies their whole `<Columns>` block, among them the sixteen
            // `InformationRegisters/*/Forms/РедактированиеИстории` that declare
            // one `ПериодСтрокой` column on their `НаборЗаписей` attribute.
            if !binding.is_empty() {
                return None;
            }
            FormAttributeAdditionalColumnsBindingKind::Attribute
        } else {
            match binding {
                [number] if number.trim().parse::<u64>().is_ok() => {
                    FormAttributeAdditionalColumnsBindingKind::Numeric
                }
                [marker] if marker.trim().parse::<i64>().is_ok_and(|value| value < 0) => {
                    FormAttributeAdditionalColumnsBindingKind::StandardMember
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

/// The four enumerations the grouping controls share, as one table each.
///
/// `HorizontalSpacing`/`VerticalSpacing`, `ChildItemsWidth` and the two
/// `Group*Align` properties are plain enumerations: the raw slot holds the
/// ordinal and `0` (spacing, width) or `3` (alignment) means "the platform omits
/// the property". Every owner reads the *same* table, so there is one table per
/// enumeration here instead of a per-owner transcription that drifts.
///
/// Measured on the UT 11.5.27.75 native tree, keyed by output path so every one
/// of the 5 201 native `Form.xml` documents is attributable:
///
/// * spacing - 26 258 `UsualGroup`, 6 807 `Page` and 5 184 `Form` observations;
///   `1->None 2->Half 3->Single 4->OneAndHalf 5->Double`, `0` absent, and the
///   map is a total function on every owner with no counter-example.  Reading it
///   per owner had left `UsualGroup` without `Single` and `Double`, `Page`
///   without every value but two, and `Form` without the property entirely.
/// * children width - `1->Equal 2->LeftWide 3->LeftWidest 4->LeftNarrow
///   5->LeftNarrowest`, `0` absent; same three owners, no counter-example.
/// * alignment - `0->Left/Top 1->Center 2->Right/Bottom`, `3` absent, over all
///   18 owner tags that carry it (109 262 observations), no counter-example.
pub(crate) fn form_item_spacing_xml(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "1" => Some("None"),
        "2" => Some("Half"),
        "3" => Some("Single"),
        "4" => Some("OneAndHalf"),
        "5" => Some("Double"),
        _ => None,
    }
}

pub(crate) fn form_children_width_xml(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "1" => Some("Equal"),
        "2" => Some("LeftWide"),
        "3" => Some("LeftWidest"),
        "4" => Some("LeftNarrow"),
        "5" => Some("LeftNarrowest"),
        _ => None,
    }
}

pub(crate) fn form_group_horizontal_align_xml(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "0" => Some("Left"),
        "1" => Some("Center"),
        "2" => Some("Right"),
        _ => None,
    }
}

pub(crate) fn form_group_vertical_align_xml(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "0" => Some("Top"),
        "1" => Some("Center"),
        "2" => Some("Bottom"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormPageXmlProperty {
    EnableContentChange,
    Title,
    TitleFont,
    ToolTip,
    ToolTipRepresentation,
    Width,
    Height,
    HorizontalStretch,
    VerticalStretch,
    Picture,
    Format,
    GroupHorizontalAlign,
    GroupVerticalAlign,
    Group,
    ChildrenAlign,
    HorizontalSpacing,
    VerticalSpacing,
    HorizontalAlign,
    VerticalAlign,
    ChildItemsWidth,
    ShowTitle,
    BackColor,
}

pub(crate) const FORM_PAGE_XML_ORDER: &[FormPageXmlProperty] = &[
    FormPageXmlProperty::EnableContentChange,
    FormPageXmlProperty::Title,
    // `TitleFont` trails `Title` and leads `ToolTip` and `Group` on the one
    // native page that carries it; it used to be written ahead of the title.
    FormPageXmlProperty::TitleFont,
    FormPageXmlProperty::ToolTip,
    FormPageXmlProperty::ToolTipRepresentation,
    // A page's geometry sits behind its title block, not in front of it.  UT
    // 11.5.27.75 native tree, 7 016 `Page` instances: `Width` (57) trails
    // `Title` (57), `ToolTip` (6), `EnableContentChange` (4) and
    // `ToolTipRepresentation` (1) and leads `HorizontalStretch` (51),
    // `VerticalStretch` (7), `Height` (4), `Picture` (4), `Group` (12),
    // `HorizontalAlign` (4), `ShowTitle` (4) and `ChildItemsWidth` (4);
    // `Height` (74) trails `Title` (72), `ToolTip` (9), `Width` (4),
    // `EnableContentChange` (4), `Visible` (1) and `Enabled` (1) and leads
    // `VerticalStretch` (41), `HorizontalStretch` (39), `ShowTitle` (38),
    // `BackColor` (34), `HorizontalAlign` (32), the two spacings (24 each),
    // `Group` (17) and `VerticalAlign` (9).  `Picture` (95) trails `Width`,
    // `HorizontalStretch` and `VerticalStretch` (4 each) and leads `Group`
    // (14), `TitleDataPath` (11) and `ChildItemsWidth` (5), so it moves behind
    // the stretch pair.  No pair is observed in both directions.
    FormPageXmlProperty::Width,
    FormPageXmlProperty::Height,
    FormPageXmlProperty::HorizontalStretch,
    FormPageXmlProperty::VerticalStretch,
    FormPageXmlProperty::Picture,
    // A page carries the same localised `<Format>` its sibling grouping
    // controls do, in option member 5 of its own tuple.  Over all 5 801 `Page`
    // records the export walks the member is the empty container `{1,0}` on
    // 5 799 and a populated one on exactly the two the platform writes a
    // `<Format>` for -- `СтраницаТовары` (in four forms) and
    // `СтраницаСертификаты` -- which are, block for block, the five native
    // `Page`/`Format` pairs in the configuration.  The member had no reader, so
    // the block was never written.
    //
    // Position: on all five, `Title` leads it (5), `Picture` leads it (1), and
    // it leads `TitleDataPath` (5), `ExtendedTooltip` (5) and `ChildItems` (5),
    // with no pair counted both ways.
    FormPageXmlProperty::Format,
    // A page's group alignment pair sits between its geometry block and its
    // `Group`, exactly where `UsualGroup` puts its own.  UT 11.5.27.75 native
    // tree, the 4 pages that carry `GroupVerticalAlign`: `Title` leads it (4)
    // and it leads `Group` (1), `HorizontalSpacing` (1), `ExtendedTooltip` (4)
    // and `ChildItems` (4), with no pair counted the other way.
    FormPageXmlProperty::GroupHorizontalAlign,
    FormPageXmlProperty::GroupVerticalAlign,
    FormPageXmlProperty::Group,
    // `ChildrenAlign` sits behind `Group` and ahead of the spacing pair, the
    // slot `UsualGroup` gives it.  The two native pages that carry it show only
    // `Title` and `VerticalStretch` in front and `ExtendedTooltip` behind, so
    // its place inside that run is fixed by the sibling table rather than
    // guessed; every position between them writes the same bytes here.
    FormPageXmlProperty::ChildrenAlign,
    // `HorizontalSpacing` then `VerticalSpacing` sit between `Group` and the
    // `*Align` pair on `Page`, which is where `UsualGroup` already puts them.
    // UT 11.5.27.75 native tree, 7 016 `Page` instances: HorizontalSpacing
    // trails Group (53), VerticalStretch (35), HorizontalStretch (34), Height
    // (24), EnableContentChange (17) and Title (87), and leads VerticalSpacing
    // (61), HorizontalAlign (30), VerticalAlign (3), ShowTitle (40), BackColor
    // (31) and ChildItems (96); VerticalSpacing repeats the same relations
    // (31/45/44/24/17/94 and 33/5/43/39/110).  No pair is observed in both
    // directions.
    FormPageXmlProperty::HorizontalSpacing,
    FormPageXmlProperty::VerticalSpacing,
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

/// A popup's `Representation`, member 4 of its nine-member option tuple.
///
/// Member 6 of the same tuple is the popup's `ShapeRepresentation`, a property
/// in its own right: over the 3 911 native popups of UT 11.5.27.75 it reads `0`
/// on 3 822, `3` on the 86 that say `None` and `2` on the 3 that say
/// `WhenActive`.  Requiring it to be `0` here therefore refused to read member 4
/// on 89 popups, three of which carry `<Representation>Text</Representation>`.
/// Member 4 alone is a total function of the native spelling on all 3 911
/// popups (`3` -> nothing 2 300 times, `1` -> `Picture` 1 163, `2` ->
/// `PictureAndText` 440, `0` -> `Text` 8), so the shape guard keeps only the
/// members that are constant across the corpus.
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

/// The two colours a `Popup` keeps in its own option tuple.
///
/// The tuple is the popup's, not the shape's, so the guard is the tuple itself:
/// wrapper `22`, tag `Popup`, nine members, member 0 spelling the tuple kind.
/// All 3 911 popups of the native UT 11.5.27.75 form dumps satisfy it, and
/// under the one colour grammar slot 7 reproduces `<BackColor>` on every one of
/// them (5 written) and slot 8 reproduces `<BorderColor>` on every one of them
/// (1 written), with the unset shape coinciding exactly with the absences.
///
/// `FormPopupSchema` reaches the same tuple behind three further equalities
/// that belong to the representation it reads, one of which -- member 6 being
/// `0` -- is the shape representation another schema reads as a property. That
/// narrower guard refuses 89 popups, and the single popup that carries a
/// `<BorderColor>` is one of them, which is why the colours read their tuple
/// through this guard instead. On the 3 822 popups both guards admit the two
/// agree exactly, and none of the 89 the narrow guard refuses carries a
/// `<BackColor>`, so nothing this schema adds contradicts what it replaces.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPopupColorSchema;

impl FormPopupColorSchema {
    pub(crate) const OPTIONS_SLOT: usize = FormPopupSchema::OPTIONS_SLOT;
    const OPTION_COUNT: usize = 9;
    const OPTION_KIND: &'static str = "7";
    pub(crate) const BACK_COLOR_OPTION_SLOT: usize = 7;
    pub(crate) const BORDER_COLOR_OPTION_SLOT: usize = 8;

    pub(crate) fn from_raw_layout(wrapper: &str, item_tag: &str, options: &[&str]) -> Option<Self> {
        (wrapper == "22"
            && item_tag == "Popup"
            && options.len() == Self::OPTION_COUNT
            && options.first().map(|field| field.trim()) == Some(Self::OPTION_KIND))
        .then_some(Self)
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
    /// The item id of a form's own command bar is `-1` on 5 200 of the 5 201
    /// forms of UT 11.5.27.75 -- and on the 5 201st it is an ordinary item id.
    /// Requiring `-1` therefore refused that one record outright and the whole
    /// `<AutoCommandBar>` block, its `<Autofill>false</Autofill>` and its
    /// fourteen buttons went unwritten.
    ///
    /// Evidence: `Documents/ЭлектроннаяСопроводительнаяВедомость/Forms/ОсновнаяФорма`
    /// carries `{22,{607,02023637-…},0,0,0,9,"ФормаКоманднаяПанель",…}` as a
    /// direct member of its form record, and the platform writes
    /// `<AutoCommandBar name="ФормаКоманднаяПанель" id="607">`.
    ///
    /// The `-1` route stays exactly as it was, so none of the 5 200 can change
    /// answer; a record with any other id is accepted only when it declares
    /// itself an auto command bar in slot 5, the same `9` the nested schema
    /// already requires.  On all 22 form records dumped for this package the
    /// two routes select the same record wherever the `-1` one selects any.
    pub(crate) fn from_raw_layout(wrapper: &str, item_id: &str, fields: &[&str]) -> Option<Self> {
        const AUTO_COMMAND_BAR_DISCRIMINATOR_SLOT: usize = 5;
        if wrapper != "22" {
            return None;
        }
        if item_id != "-1"
            && fields
                .get(AUTO_COMMAND_BAR_DISCRIMINATOR_SLOT)
                .map(|field| field.trim())
                != Some("9")
        {
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
    children_align: Option<&'static str>,
    child_items_width: Option<&'static str>,
    horizontal_spacing: Option<&'static str>,
    vertical_spacing: Option<&'static str>,
    scroll_on_compress: Option<bool>,
}

impl FormPageSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;
    /// Option slots of the spacing pair and the children width on a `Page`.
    /// UT 11.5.27.75 native tree, 6 807 traced pages: slot 10 is a total
    /// function for `HorizontalSpacing` (95 present), slot 11 for
    /// `VerticalSpacing` (110) and slot 3 for `ChildItemsWidth` (39), all three
    /// under the shared enumeration tables and with no counter-example.
    const HORIZONTAL_SPACING_OPTION_SLOT: usize = 10;
    const VERTICAL_SPACING_OPTION_SLOT: usize = 11;
    const CHILD_ITEMS_WIDTH_OPTION_SLOT: usize = 3;
    const SCROLL_ON_COMPRESS_OPTION_SLOT: usize = 15;

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
            // The alignment code runs `0 -> Left`, `1 -> Center`,
            // `2 -> Right`, `3 -> nothing`, the same table the grouping
            // controls use; only `Center` used to be decoded, so the 37 left-
            // and 47 right-aligned pages were dropped.  UT 11.5.27.75, all
            // 6 983 traced pages: option member 12 is a total function of the
            // platform answer with those four codes and no other, and no code
            // maps to two answers.
            horizontal_align: match options.get(12).map(|field| field.trim()) {
                Some("0") => Some("Left"),
                Some("1") => Some("Center"),
                Some("2") => Some("Right"),
                _ => None,
            },
            // The vertical code runs `0 -> Top`, `1 -> Center`, `2 -> Bottom`,
            // `3 -> nothing`, the same four-code table the horizontal one uses;
            // `Top` used to be missing, which dropped the three top-aligned
            // pages.  UT 11.5.27.75, all 7 016 traced pages: option member 13
            // is a total function of the platform answer -- `3` on the 6 779
            // pages without the element and `0`/`1`/`2` on exactly the
            // 3/223/11 that say `Top`, `Center` and `Bottom`.
            vertical_align: match options.get(13).map(|field| field.trim()) {
                Some("0") => Some("Top"),
                Some("1") => Some("Center"),
                Some("2") => Some("Bottom"),
                _ => None,
            },
            // `ChildrenAlign` rides option member 14 under the same six-code
            // table the `UsualGroup` tuple uses one member further along: over
            // all 7 016 traced pages the member is `0` on the 7 014 without the
            // element and `1`/`6` on exactly the two that say `None` and
            // `TitlesLeftDataAuto`, with no code mapping to two answers.
            children_align: match options.get(14).map(|field| field.trim()) {
                Some("1") => Some("None"),
                Some("2") => Some("ItemsLeftTitlesLeft"),
                Some("3") => Some("ItemsRightTitlesLeft"),
                Some("4") => Some("ItemsLeftTitlesRight"),
                Some("5") => Some("ItemsRightTitlesRight"),
                Some("6") => Some("TitlesLeftDataAuto"),
                _ => None,
            },
            child_items_width: options
                .get(Self::CHILD_ITEMS_WIDTH_OPTION_SLOT)
                .and_then(|field| form_children_width_xml(field)),
            horizontal_spacing: options
                .get(Self::HORIZONTAL_SPACING_OPTION_SLOT)
                .and_then(|field| form_item_spacing_xml(field)),
            vertical_spacing: options
                .get(Self::VERTICAL_SPACING_OPTION_SLOT)
                .and_then(|field| form_item_spacing_xml(field)),
            // `ScrollOnCompress` lives in the page's own option tuple, not in
            // the top-level slot the reader used to sample: UT 11.5.27.75, all
            // 6 983 traced pages, option member 15 is `1` on exactly the 104
            // pages the platform writes `<ScrollOnCompress>true` on and `0` on
            // the other 6 879, with no counter-example.  The former rule (top
            // level slot 11, gated on slot 8 opening a brace) missed 103 of the
            // 104 and invented 15 the platform never writes.
            scroll_on_compress: match options
                .get(Self::SCROLL_ON_COMPRESS_OPTION_SLOT)
                .map(|field| field.trim())
            {
                Some("1") => Some(true),
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

    pub(crate) const fn children_align(self) -> Option<&'static str> {
        self.children_align
    }

    pub(crate) const fn child_items_width(self) -> Option<&'static str> {
        self.child_items_width
    }

    pub(crate) const fn horizontal_spacing(self) -> Option<&'static str> {
        self.horizontal_spacing
    }

    pub(crate) const fn vertical_spacing(self) -> Option<&'static str> {
        self.vertical_spacing
    }

    pub(crate) const fn scroll_on_compress(self) -> Option<bool> {
        self.scroll_on_compress
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
    FormUsualGroupHeaderXmlProperty::TitleTextColor,
    FormUsualGroupHeaderXmlProperty::TitleFont,
    FormUsualGroupHeaderXmlProperty::ToolTip,
    FormUsualGroupHeaderXmlProperty::ToolTipRepresentation,
    // `Shortcut` closes the header: on the 10 native groups that carry it, it
    // trails `Title` (10), `ToolTipRepresentation` (1), `ReadOnly` (1) and
    // `EnableContentChange` (1) and leads `ShowTitle` (9), `Behavior` (9),
    // `Representation` (8), `Group` (4), `ChildItemsWidth` (2), `ThroughAlign`
    // (1) and `HorizontalStretch` (1).  It never shares a group with
    // `TitleTextColor`, `TitleFont` or `ToolTip`, so the end of the header is
    // the nearest position that satisfies every observed pair.
    FormUsualGroupHeaderXmlProperty::Shortcut,
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
    HiddenStateTitleBackColor,
    ThroughAlign,
    CurrentRowUse,
}

pub(crate) const FORM_USUAL_GROUP_XML_ORDER: &[FormUsualGroupXmlProperty] = &[
    // `Enabled` leads `ReadOnly` on all 3 native groups that carry both, and
    // both lead `EnableContentChange` (4 each); never the other way round.
    FormUsualGroupXmlProperty::Enabled,
    FormUsualGroupXmlProperty::ReadOnly,
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
    // The one native usual group that carries `HiddenStateTitleBackColor`
    // writes it behind `Title`, `Behavior`, `Representation` and `ShowTitle`
    // and ahead of `ExtendedTooltip` and `ChildItems`; it shares no group with
    // `BackColor`, `ThroughAlign` or `CurrentRowUse`, so it joins their site
    // rather than opening a second one.
    FormUsualGroupXmlProperty::HiddenStateTitleBackColor,
    FormUsualGroupXmlProperty::ThroughAlign,
    // The one native usual group that carries `CurrentRowUse` writes it
    // behind `Title`, `HorizontalStretch`, `GroupHorizontalAlign`, `Group`,
    // `Representation`, `ShowTitle` and `BackColor`, and ahead of
    // `ExtendedTooltip` and `ChildItems`.
    FormUsualGroupXmlProperty::CurrentRowUse,
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
            Self::BackColor
            | Self::HiddenStateTitleBackColor
            | Self::ThroughAlign
            | Self::CurrentRowUse => FormUsualGroupXmlAnchor::AfterShowTitle,
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
    /// Decoded through the one shared alignment table, so this owner cannot
    /// drift away from the other seventeen that read the same enumeration.
    pub(crate) fn from_raw_value(raw: &str) -> Option<Self> {
        match form_group_vertical_align_xml(raw)? {
            "Top" => Some(Self::Top),
            "Center" => Some(Self::Center),
            "Bottom" => Some(Self::Bottom),
            _ => None,
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

impl FormUsualGroupSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;
    const GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET: usize = 3;
    const GROUP_VERTICAL_ALIGN_REVERSE_OFFSET: usize = 2;
    /// UT 11.5.27.75 native tree, 26 258 traced groups: option slot 15 is a
    /// total function for `HorizontalSpacing` (825 present), slot 16 for
    /// `VerticalSpacing` (732) and slot 2 for `ChildItemsWidth` (472), all under
    /// the shared enumeration tables and with no counter-example.
    const HORIZONTAL_SPACING_OPTION_SLOT: usize = 15;
    const VERTICAL_SPACING_OPTION_SLOT: usize = 16;
    const CHILD_ITEMS_WIDTH_OPTION_SLOT: usize = 2;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        direct_discriminator: Option<&str>,
        options: &[&str],
    ) -> Option<Self> {
        // A `UsualGroup` is the same `30 + 2n` layout family that `Page` and
        // `Popup` already state structurally, so it is tested structurally here
        // too.  The previous guard listed the fourteen even counts that happened
        // to be in view when it was written and so rejected 48 of the 26 258
        // native groups outright - every count of 56, 58, 62, 64, 66, 68, 70,
        // 72, 74, 76, 80, 86 and 88 - while admitting 60.  Rejecting the item
        // discards the whole property bag, not just one property: one of the 48
        // carries `GroupVerticalAlign`, `HorizontalSpacing`, `VerticalSpacing`
        // and `ChildItemsWidth` at once.
        (field_count >= 30
            && (field_count - 30).is_multiple_of(2)
            && matches!(
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
            // The two right-titled codes used to be missing from the table.
            // UT 11.5.27.75, all 26 672 traced `UsualGroup` items: option
            // member 20 is a total function of the platform answer -- `0` on
            // the 26 629 groups without a `<ChildrenAlign>` and `1`..`6` on the
            // 9/30/7/1/4/2 that say `None`, `ItemsLeftTitlesLeft`,
            // `ItemsRightTitlesLeft`, `ItemsLeftTitlesRight`,
            // `ItemsRightTitlesRight` and `TitlesLeftDataAuto`, with no code
            // mapping to two answers.
            children_align: options.get(20).and_then(|field| match field.trim() {
                "1" => Some("None"),
                "2" => Some("ItemsLeftTitlesLeft"),
                "3" => Some("ItemsRightTitlesLeft"),
                "4" => Some("ItemsLeftTitlesRight"),
                "5" => Some("ItemsRightTitlesRight"),
                "6" => Some("TitlesLeftDataAuto"),
                _ => None,
            }),
            horizontal_spacing: options
                .get(Self::HORIZONTAL_SPACING_OPTION_SLOT)
                .and_then(|field| form_item_spacing_xml(field)),
            vertical_spacing: options
                .get(Self::VERTICAL_SPACING_OPTION_SLOT)
                .and_then(|field| form_item_spacing_xml(field)),
            child_items_width: options
                .get(Self::CHILD_ITEMS_WIDTH_OPTION_SLOT)
                .and_then(|field| form_children_width_xml(field)),
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
        form_group_horizontal_align_xml(fields.get(slot)?)
    }

    fn group_vertical_align(self, fields: &[&str]) -> Option<FormUsualGroupGroupVerticalAlign> {
        let slot = fields
            .len()
            .checked_sub(Self::GROUP_VERTICAL_ALIGN_REVERSE_OFFSET)?;
        FormUsualGroupGroupVerticalAlign::from_raw_value(fields.get(slot)?)
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
    transparent_pixel: Option<(i64, i64)>,
}

impl FormPictureValueSchema {
    pub(crate) fn from_raw_layout(value: &[&str]) -> Option<Self> {
        if value.first().map(|field| field.trim()) != Some("4")
            || value.get(3).map(|field| field.trim()) != Some("\"\"")
        {
            return None;
        }
        // Members 4 and 5 are the transparent pixel's coordinates, `-1, -1`
        // when the picture declares none. The pair was read as a fixed `-1,
        // -1` prologue, so a record that does declare a pixel failed the shape
        // test and the whole picture went unwritten -- not just the pixel.
        //
        // Evidence: UT 11.5.27.75,
        // `Catalogs/ИсточникиДанныхПланирования/Forms/ФормаЗаполнения`
        // `PictureField` `ПравилоЗаполненияПрисоединять`, whose picture record
        // reads `{4,3,{0},"",7,4,1,{<payload>},0,""}` and which the platform
        // writes as `<xr:Abs>ValuesPicture.bmp</xr:Abs>`,
        // `<xr:LoadTransparent>true</xr:LoadTransparent>` and
        // `<xr:TransparentPixel x="7" y="4"/>`. The same pair in the same two
        // members is what the `ExtPicture` writer already reads for the
        // stand-alone common pictures.
        let transparent_pixel = match (
            value.get(4).map(|field| field.trim()),
            value.get(5).map(|field| field.trim()),
        ) {
            (Some("-1"), Some("-1")) => None,
            (Some(x), Some(y)) => Some((x.parse().ok()?, y.parse().ok()?)),
            _ => return None,
        };
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
            transparent_pixel,
        })
    }

    pub(crate) const fn kind(self) -> FormPictureValueKind {
        self.kind
    }

    pub(crate) const fn load_transparent(self) -> bool {
        self.load_transparent
    }

    pub(crate) const fn transparent_pixel(self) -> Option<(i64, i64)> {
        self.transparent_pixel
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

    /// Slot of the header container a `ColumnGroup` keeps its header properties
    /// in, and the slot of the picture record inside that container.
    ///
    /// A `ColumnGroup` does not carry the header picture at the flat index the
    /// four field kinds use - it has no such slot - so the record was never
    /// reached and the element was never written.  Over all 3 008 native
    /// `ColumnGroup` items the container at slot 20 is invariably a
    /// twelve-field record led by `2`, and slot 5 of it is a total function of
    /// the platform's answer: the empty picture record on exactly the 2 989 that
    /// carry no `<HeaderPicture>`, and a reference record on exactly the 19 that
    /// do, with no observation on an ambiguous key.
    pub(crate) const COLUMN_GROUP_CONTAINER_SLOT: usize = 20;
    pub(crate) const COLUMN_GROUP_CONTAINER_FIELDS: usize = 12;
    pub(crate) const COLUMN_GROUP_PICTURE_SLOT: usize = 5;

    pub(crate) fn from_column_group_layout(
        wrapper: &str,
        item_tag: &str,
        container: &[&str],
        value: &[&str],
    ) -> Option<Self> {
        if wrapper != "22"
            || item_tag != "ColumnGroup"
            || container.len() != Self::COLUMN_GROUP_CONTAINER_FIELDS
            || container.first().map(|field| field.trim()) != Some("2")
        {
            return None;
        }
        let value = FormPictureValueSchema::from_raw_layout(value)?;
        Some(Self {
            picture_slot: Self::COLUMN_GROUP_PICTURE_SLOT,
            value,
        })
    }

    /// The footer picture of the same four field kinds, read from the slot
    /// directly behind the header one.
    ///
    /// The header picture already establishes that a field carries its two
    /// column pictures as adjacent picture records; the footer is the second
    /// of the pair.  Evidence, UT 11.5.27.75: slot `30 + offset` holds the
    /// platform's "empty" picture record on every field of the corpus but two,
    /// and a reference record on exactly the two the platform writes
    /// `<FooterPicture>` on - `CommonForms/РаспределениеРасходовНаПоступления`
    /// item `СписокДокументовВес` (`CommonPicture.Предупреждение32`) and
    /// `DataProcessors/ТорговыеПредложения/Forms/ФормированиеЗаказов` item
    /// `КонтрагентыСуммаСНДС` (`CommonPicture.Сумма`), both with the
    /// transparency flag clear.  The record was never read, so the element was
    /// never written.
    pub(crate) fn from_footer_layout(
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
            picture_slot: 30 + top_level_offset,
            value,
        })
    }

    pub(crate) const fn picture_slot(self) -> usize {
        self.picture_slot
    }

    /// The whole picture value, so a reader of this schema answers about the
    /// transparent pixel from the same record it answers about the reference
    /// and the transparency flag.
    pub(crate) const fn picture(self) -> FormPictureValueSchema {
        self.value
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormFieldHeaderPictureXmlProperty {
    Value,
    LoadTransparent,
    TransparentPixel,
}

/// `<xr:TransparentPixel>` closes a picture element, behind
/// `<xr:LoadTransparent>`, exactly as the stand-alone `ExtPicture` writer and
/// the form command's own picture writer already spell it.  Over the 63 449
/// picture elements of the five reference trees no picture writes the pixel
/// anywhere but last.
pub(crate) const FORM_FIELD_HEADER_PICTURE_XML_ORDER: &[FormFieldHeaderPictureXmlProperty] = &[
    FormFieldHeaderPictureXmlProperty::Value,
    FormFieldHeaderPictureXmlProperty::LoadTransparent,
    FormFieldHeaderPictureXmlProperty::TransparentPixel,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootMobileDeviceCommandBarContentSchema {
    item_count: usize,
}

impl FormRootMobileDeviceCommandBarContentSchema {
    /// The content tuple sits at trailer slot 22 plus the trailer's declared
    /// optional-block count.
    ///
    /// The block the count introduces is the form root's built-in
    /// Navigator/quick-search item. Evidence: ERP УХ 3.2.12.6, four native
    /// forms spanning `BusinessProcesses`/`Catalogs`/`CommonForms`
    /// (`Задание/Forms/ФормаСписка`, `Валюты/Forms/ФормаСписка`,
    /// `ВерсииФайлов/Forms/ФормаВыбора`, `ФормаОтчета`): the shared
    /// count-list scan (`form_root_child_items_tail_start_at`,
    /// `mssql_dump::form_body`) validates cleanly at `fields.len() - 25` on
    /// all four where `fields.len() - 24` finds no valid count-list at all,
    /// and in every case the resulting trailer's second-to-last field is a
    /// `{50,1,...}`-shaped content tuple identical in shape to the
    /// already-working 24-trailer case. 84 native ERP УХ forms carry
    /// `<MobileDeviceCommandBarContent>`; the 24-only trailer this schema
    /// previously required matched none of the 25-trailer forms among them,
    /// silently dropping the whole block.
    pub(crate) const CONTENT_TRAILER_SLOT: usize = 22;

    /// The content block is validated against the trailer's declared
    /// optional-block count, not against a fixed trailer length.
    ///
    /// The previous gate required exactly 24 trailer members, which is the
    /// count-0 shape БСП, БСП демо, УТ and WMS use -- where the property is
    /// already byte-exact on all 210 forms that carry it. ERP УХ declares a
    /// count of 1, so its trailer runs to 25 and the gate rejected every one
    /// of its forms, dropping the block entirely rather than reading it at the
    /// wrong offset.
    ///
    /// The block leads with its own copy of the root discriminator -- `{50,0}`
    /// under root `50`, `{49,0}` under root `49` -- so `content_kind` is
    /// checked against the root rather than against a literal `50`.
    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer: &[&str],
        content_kind: Option<&str>,
        content_field_count: usize,
        declared_item_count: usize,
        typed_item_count: usize,
    ) -> Option<Self> {
        if !matches!(root_discriminator, Some("49") | Some("50")) {
            return None;
        }
        form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        let expected_field_count = declared_item_count.checked_mul(2)?.checked_add(2)?;
        (content_kind == root_discriminator
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
    TitleHeight,
    Hyperlink,
    GroupHorizontalAlign,
    GroupVerticalAlign,
    HorizontalAlign,
    VerticalAlign,
    BackColor,
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
    // `TitleHeight` trails `Title` (2 of the 3 native tooltips that carry it)
    // and `AutoMaxWidth` (1); nothing else shares a tooltip with it.
    FormExtendedTooltipXmlProperty::TitleHeight,
    FormExtendedTooltipXmlProperty::Hyperlink,
    FormExtendedTooltipXmlProperty::GroupHorizontalAlign,
    // The alignment run of a tooltip is `GroupHorizontalAlign`,
    // `GroupVerticalAlign`, `HorizontalAlign`, `VerticalAlign`.  UT 11.5.27.75
    // native tree: `GroupHorizontalAlign` leads `GroupVerticalAlign` (3
    // co-occurrences) and `HorizontalAlign` (1), with no counter-example; the
    // three remaining pairs never share a tooltip, so their order is fixed by
    // the two that do rather than by the field family's opposite run.
    FormExtendedTooltipXmlProperty::GroupVerticalAlign,
    FormExtendedTooltipXmlProperty::HorizontalAlign,
    FormExtendedTooltipXmlProperty::VerticalAlign,
    // A tooltip's `BackColor` closes its property block, immediately ahead of
    // `Events`.  Both native tooltips that carry one pin it from a different
    // side and neither is contradicted: one reads `TextColor`, `Title`,
    // `Hyperlink`, `BackColor` and the other `BackColor`, `Events`.
    FormExtendedTooltipXmlProperty::BackColor,
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
    group_vertical_align_slot: usize,
    horizontal_align_option_slot: usize,
    title_height_option_slot: usize,
    vertical_align_option_slot: usize,
    title_values_slot: usize,
    title_formatted_slot: usize,
}

impl FormExtendedTooltipSchema {
    pub(crate) const OPTIONS_SLOT: usize = 18;
    pub(crate) const TITLE_SLOT: usize = 22;
    pub(crate) const EVENT_OPTION_SLOT: usize = 5;
    /// The option slot immediately behind the event record carries the
    /// tooltip's `BackColor`, in the same three-member `{3, space, payload}`
    /// shape every control colour uses.  Census of all 206 891 traced
    /// `ExtendedTooltip` records of UT 11.5.27.75: exactly two option slots in
    /// the whole configuration hold a colour that is not the platform's
    /// "unset" encoding, both of them this one, and they are exactly the two
    /// tooltips the platform writes `<BackColor>` on -- `style:ToolTipBackColor`
    /// and `#FFDCDC`, value for value.
    pub(crate) const BACK_COLOR_OPTION_SLOT: usize = 6;
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
            // The tooltip's group alignment pair sits in adjacent top-level
            // slots, the vertical one directly behind the horizontal one.
            // Measured on all 206 891 `ExtendedTooltip` records the export
            // walks in UT 11.5.27.75: slot 31 reads `3` on every one of the
            // 206 879 tooltips the platform writes no `<GroupVerticalAlign>`
            // on, and `0`/`1`/`2` on exactly the 1/9/2 it writes `Top`,
            // `Center` and `Bottom` on, with no counter-example.
            group_vertical_align_slot: 31,
            // Option member 2 carries `<HorizontalAlign>`: `0` on all 206 881
            // tooltips without the element and `1`/`2`/`3` on exactly the
            // 2/3/5 that say `Center`, `Right` and `Auto`.  `Left` is the
            // suppressed default and is never written, which is why `0` maps
            // to no element rather than to a spelling.
            horizontal_align_option_slot: 2,
            // Option member 4 carries `<TitleHeight>` as its own value: `0` on
            // all 206 888 tooltips without the element and `2`/`3` on exactly
            // the 2/1 that write those heights.
            title_height_option_slot: 4,
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

    pub(crate) const fn group_vertical_align_slot(self) -> usize {
        self.group_vertical_align_slot
    }

    pub(crate) const fn horizontal_align_option_slot(self) -> usize {
        self.horizontal_align_option_slot
    }

    pub(crate) const fn title_height_option_slot(self) -> usize {
        self.title_height_option_slot
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
    Embossed,
    Underline,
    Overline,
    Double,
}

impl FormControlBorderStyle {
    /// Style codes of the seven-member control-border tuple, read off the whole
    /// native UT 11.5.27.75 tree against the platform's own
    /// `<v8ui:style xsi:type="v8ui:ControlBorderType">`: member 3 is a total
    /// function of the spelling on every owner that carries one -- `0`
    /// `WithoutBorder` (468), `1` `Single` (18), `2` `Embossed` (3), `4`
    /// `Underline` (10), `7` `Overline` (3) and `200` `Double` (4) -- and no
    /// code maps to two spellings.  `Embossed` and `Double` used to be missing,
    /// which dropped seven borders outright.
    pub(crate) fn from_raw_code(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::WithoutBorder),
            "1" => Some(Self::Single),
            "2" => Some(Self::Embossed),
            "4" => Some(Self::Underline),
            "7" => Some(Self::Overline),
            "200" => Some(Self::Double),
            _ => None,
        }
    }

    pub(crate) const fn raw_code(self) -> &'static str {
        match self {
            Self::WithoutBorder => "0",
            Self::Single => "1",
            Self::Embossed => "2",
            Self::Underline => "4",
            Self::Overline => "7",
            Self::Double => "200",
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value.trim() {
            "WithoutBorder" => Some(Self::WithoutBorder),
            "Single" => Some(Self::Single),
            "Embossed" => Some(Self::Embossed),
            "Underline" => Some(Self::Underline),
            "Overline" => Some(Self::Overline),
            "Double" => Some(Self::Double),
            _ => None,
        }
    }

    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::WithoutBorder => "WithoutBorder",
            Self::Single => "Single",
            Self::Embossed => "Embossed",
            Self::Underline => "Underline",
            Self::Overline => "Overline",
            Self::Double => "Double",
        }
    }
}

/// A control border as the platform writes it: the style spelling and the
/// `width` attribute that rides member 4 of the same tuple.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormControlBorder {
    pub(crate) style: FormControlBorderStyle,
    pub(crate) width: u32,
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
            "CalendarField" if top_level_offset == 0 => Some(FormFieldSchema::OPTIONS_BASE_SLOT),
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
            // A calendar field carries the same tuple in member 18 of its own
            // option tuple: over the 5 native `CalendarField` records the
            // export walks, member 3 of that tuple is `1` on the 4 that carry
            // no `<Border>` and `0` on the one that writes `WithoutBorder`,
            // which is the same `Single`-is-the-default shape a picture field
            // has.
            ("37", 59, "CalendarField", 0, Some("8"), 24, Some("6")) => {
                (18, FormControlBorderStyle::Single)
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

    /// Member 4 of the tuple is the `width` attribute the platform writes on
    /// the element, not a shape constant: over the whole native UT 11.5.27.75
    /// tree it reads `1` on every border written as `width="1"` and `5`, `3`
    /// and `0` on exactly the three `width="5"`, the one `width="3"` and the
    /// one `width="0"` label decorations.  Pinning it to `1` had made the
    /// reader refuse those five borders and had hard-coded the attribute.
    pub(crate) fn tuple_border(self, tuple: &[&str]) -> Option<FormControlBorder> {
        if tuple.len() != 7
            || tuple.first().map(|field| field.trim()) != Some("3")
            || tuple.get(1).map(|field| field.trim()) != Some("0")
            || tuple.get(2).map(|field| field.trim()) != Some("{0}")
            || tuple.get(5).map(|field| field.trim()) != Some("0")
            || uuid::Uuid::parse_str(tuple.get(6)?.trim()).is_err()
        {
            return None;
        }
        Some(FormControlBorder {
            style: FormControlBorderStyle::from_raw_code(tuple.get(3)?)?,
            width: tuple.get(4)?.trim().parse::<u32>().ok()?,
        })
    }

    /// The platform writes the element when the style is not the owner's
    /// default *or* the width is not `1`: on the whole native tree every
    /// `<Border>` satisfies one of the two and every record satisfying neither
    /// carries none, with no counter-example on any of the five owners.
    pub(crate) fn non_default_tuple_border(self, tuple: &[&str]) -> Option<FormControlBorder> {
        self.tuple_border(tuple)
            .filter(|border| border.style != self.default_style || border.width != 1)
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
    /// `BorderColor` of the decoration, inside its own option tuple.
    pub(crate) const BORDER_COLOR_SLOT: usize = 6;

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

    /// The drag pair of the decoration option tuple.  Over all 3 725
    /// `PictureDecoration` option tuples the export walks -- every one of them
    /// a thirteen-member record opening with `4` -- slot 8 holds `1` on exactly
    /// the 2 decorations the platform writes `<EnableStartDrag>true</...>` on
    /// and slot 9 holds `1` on exactly the 4 it writes `<EnableDrag>true</...>`
    /// on; both slots hold `0` on every other decoration and no other code
    /// occurs in either.
    pub(crate) fn enable_start_drag_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(8)
    }

    pub(crate) fn enable_drag_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(9)
    }

    /// `Zoomable` flag of the decoration option tuple.  Over all 3 725
    /// `PictureDecoration` option tuples the export walks, the slot holds `1`
    /// on exactly the 7 the platform writes `<Zoomable>true</Zoomable>` on and
    /// `0` on the other 3 718; the platform never writes the element with any
    /// other value.
    pub(crate) fn zoomable_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(4)
    }

    /// `ImageScale` percentage of the decoration option tuple.  Over the same
    /// 3 725 tuples the slot holds `100` on the 3 718 items that carry no
    /// `<ImageScale>` and, on the 7 that do, exactly the number the platform
    /// writes -- `200` six times and `108` once.  `100` is therefore the
    /// unwritten default, not an absence.
    pub(crate) fn image_scale_option_slot(self, options: &[&str]) -> Option<usize> {
        self.option_tuple_is_exact(options).then_some(12)
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
            // Slots 32 and 33 are total functions for the two alignment
            // properties on all 3 666 traced picture decorations under the
            // shared tables (110 and 417 present).  Reading vertical alignment
            // without the `Bottom` ordinal had dropped 28 of those 417.
            group_horizontal_align: fields
                .get(32)
                .and_then(|field| form_group_horizontal_align_xml(field)),
            group_vertical_align: fields
                .get(33)
                .and_then(|field| form_group_vertical_align_xml(field)),
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
            // The three list additions keep the importance code in the last
            // member of their own 24-member wrapper-`5` record, exactly as the
            // wrapper-`22` containers do.  Across all 13 942 of them in UT
            // 11.5.27.75 the slot reads `0` on every addition the platform
            // gives no `DisplayImportance`, `1` on the one it marks
            // `VeryHigh` and `5` on the two it marks `VeryLow`.
            (
                "5",
                24,
                "SearchStringAddition" | "ViewStatusAddition" | "SearchControlAddition",
                0,
            ) => 23,
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

    /// The decoration keeps its border colour in option slot 7, one slot ahead
    /// of the back colour it already reads from slot 6.
    ///
    /// Read over all 11 550 native `LabelDecoration` items of UT 11.5.27.75
    /// under the one colour grammar, the slot is a total function of the
    /// platform's `<BorderColor>`: the unset shape on the 11 545 that carry no
    /// element and a readable colour on all five that do.
    pub(crate) const BORDER_COLOR_OPTION_SLOT: usize = 7;

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
            // 11 463 traced label decorations, 438 carrying the property: slot 33
            // is a total function under the shared table.  Omitting the `Top`
            // ordinal had dropped 44 of the 438.
            group_vertical_align: fields
                .get(self.group_vertical_align_slot())
                .and_then(|field| form_group_vertical_align_xml(field)),
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
    /// Option slot the block's own declared revision keeps `CheckBoxType` in.
    check_box_type_option_slot: usize,
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
            // Slot 7 reads `0` on all five `PDFDocumentField` items of UT
            // 11.5.27.75 and the platform writes `<TitleLocation>None</TitleLocation>`
            // on all five, the same code-to-value pair the other field kinds use.
            "PDFDocumentField" => "20",
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
                Some("0" | "1" | "2")
            ))
        .then_some(Self)
    }

    /// The column group's `FixingInTable` runs the same three codes the field
    /// family does -- `0` nothing, `1` `Left`, `2` `Right`.  UT 11.5.27.75, all
    /// 3 008 `ColumnGroup` records the export walks: option member 11 is `0` on
    /// the 2 980 groups without the element and `1`/`2` on exactly the 26
    /// `Left` and 2 `Right`, with no counter-example.  `2` used to be rejected
    /// by the layout gate itself, which threw away the whole schema -- and with
    /// it every other property -- on those two groups.
    pub(crate) fn fixing_in_table(self, options: &[&str]) -> Option<FormFixingInTable> {
        FormFixingInTable::from_field_raw_value(options.get(Self::FIXING_IN_TABLE_OPTION_SLOT)?)
    }

    pub(crate) const fn fixing_in_table_raw_code(
        self,
        value: FormFixingInTable,
    ) -> Option<&'static str> {
        Some(value.field_raw_code())
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
    html_document_options: bool,
    picture_field_options: bool,
    title_slot: usize,
    footer_text_slot: usize,
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
    auto_max_width_option_slot: Option<usize>,
    equal_items_width_option_slot: Option<usize>,
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
    pub(crate) view_scaling_mode: Option<&'static str>,
    pub(crate) show_groups: Option<bool>,
    pub(crate) drawing_selection_show_mode: Option<&'static str>,
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
const FORM_SPREADSHEET_URL_PROCESSING_EVENT_UUID: &str = "06d41ccc-4e8a-46f8-aeff-b3303cf753d2";
const FORM_SPREADSHEET_BEFORE_PRINT_EVENT_UUID: &str = "61455593-0982-4415-bc2e-2e8722a7abd0";
const FORM_GRAPHICAL_SCHEMA_ON_ACTIVATE_EVENT_UUID: &str = "83c14f85-ab1f-4c77-bd3b-81970b72543b";
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

    /// What identifies a `Pages` event container is the container's own shape,
    /// and nothing about the two members that are properties in their own
    /// right.
    ///
    /// Census of all 2 687 native `Pages` items of UT 11.5.27.75: every one of
    /// them has a six-member container whose slot 0 is `4`, slot 3 is `2` and
    /// slot 4 is `0`. Slots 1 and 5 vary -- 1 999 read `0`/`0`, 631 read
    /// `1`/`1`, 51 read `1`/`6`, four read `2`/`2`, one `3`/`3` and one `5`/`5`
    /// -- and under this guard the event record at slot 2 is a total function
    /// of the platform's `<Events>` on all 2 687: it decodes to no event on the
    /// 2 487 that carry no element and to exactly the written handler on the
    /// 200 that do, the 51 items whose two slots disagree included (41 without
    /// an event, 10 with). Requiring slot 5 to repeat slot 1 is what lost those
    /// ten `OnCurrentPageChange` handlers.
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
            && container.get(3).map(|field| field.trim()) == Some("2")
            && container.get(4).map(|field| field.trim()) == Some("0"))
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
                // A collection that names one member this table does not know
                // is discarded whole, so a single missing identifier costs
                // every event beside it. These two were the missing ones: the
                // corpus writes `06d41ccc` beside the already-named
                // `DetailProcessing` and `61455593` beside the already-named
                // `Selection`, and the platform prints `URLProcessing` and
                // `BeforePrint` for them.
                (FORM_SPREADSHEET_URL_PROCESSING_EVENT_UUID, "URLProcessing"),
                (FORM_SPREADSHEET_BEFORE_PRINT_EVENT_UUID, "BeforePrint"),
            ],
            FormChildItemEventCollectionOwner::CalendarField => &[
                (FORM_CALENDAR_ON_PERIOD_OUTPUT_EVENT_UUID, "OnPeriodOutput"),
                (FORM_CALENDAR_SELECTION_EVENT_UUID, "Selection"),
            ],
            FormChildItemEventCollectionOwner::GraphicalSchemaField => &[
                (FORM_GRAPHICAL_SCHEMA_SELECTION_EVENT_UUID, "Selection"),
                // Same all-or-nothing rule: the corpus's only graphical-schema
                // collection with two members names `83c14f85` beside the
                // already-named `Selection`, and the platform prints
                // `OnActivate` for it.
                (FORM_GRAPHICAL_SCHEMA_ON_ACTIVATE_EVENT_UUID, "OnActivate"),
            ],
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

/// The `<Output>` code a form item stores, under the one table every item kind
/// that writes the element shares: `0` writes nothing, `1` writes `Enable`,
/// `2` writes `Disable`.
///
/// Evidence: UT 11.5.27.75, equality of sets per item kind against every
/// `<Output>` the platform writes anywhere in the configuration -- `Table` slot
/// 40 (4 536 / 5 / 1), `SpreadSheetDocumentField` option 12 (144 records with
/// `0` and no element, 3 with `1` and `Enable`, 1 with `2` and `Disable`) and
/// `HTMLDocumentField` option 4 (171 / 4 / 3), each with no record holding a
/// code the platform writes nothing for and no element written without it.
pub(crate) fn form_output_code(value: Option<&str>) -> Option<&'static str> {
    match value.map(str::trim) {
        Some("1") => Some("Enable"),
        Some("2") => Some("Disable"),
        _ => None,
    }
}

impl FormFieldSchema {
    pub(crate) const OPTIONS_BASE_SLOT: usize = 39;

    /// The `<Output>` of an `HTMLDocumentField`, option member 4 of its own
    /// 13-member tuple.  The member has no meaning in any other field kind's
    /// tuple, which is why the schema gates it on the kind rather than reading
    /// the same index everywhere.
    pub(crate) fn html_document_output(self, options: &[&str]) -> Option<&'static str> {
        self.html_document_options
            .then(|| form_output_code(options.get(4).copied()))
            .flatten()
    }

    pub(crate) const fn options_slot(self) -> usize {
        Self::OPTIONS_BASE_SLOT + self.top_level_offset
    }

    /// The field's `TitleBackColor`, a top-level slot carrying the same
    /// three-member `{3, space, payload}` value every control colour uses, and
    /// shifted by the same head offset the options slot is.
    ///
    /// Census of every colour-bearing item record of UT 11.5.27.75 -- all
    /// 11 099 of them, scanned at every top-level and every nested slot: this
    /// coordinate holds a colour on exactly two items, and they are exactly
    /// the two the platform writes `<TitleBackColor>` on, value for value. No
    /// other item in the configuration holds anything but the platform's
    /// "unset" encoding here.
    pub(crate) const TITLE_BACK_COLOR_BASE_SLOT: usize = 33;

    pub(crate) const fn title_back_color_slot(self) -> usize {
        Self::TITLE_BACK_COLOR_BASE_SLOT + self.top_level_offset
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
        // The border colour rides the option tuple of every field kind that
        // writes one, and each of the three kinds added here was read off the
        // whole native UT 11.5.27.75 tree as a total function of the platform's
        // `<BorderColor>` under the one colour grammar: option slot 13 on all
        // 16 203 `LabelField` items (4 written), slot 11 on all 1 209
        // `PictureField` items (24 written), slot 8 on all 41
        // `FormattedDocumentField` items (7 written). In each case the unset
        // shape `{3,4,{0}}` coincides exactly with the items that carry no
        // element and no other shape ever coincides with an absence.
        let (discriminator, options_len, options_kind, text, back, border) = match item_tag {
            "LabelField" => ("1", 20, "11", Some(8), Some(9), Some(13)),
            "InputField" => ("2", 66, "36", Some(37), Some(38), Some(39)),
            "CheckBoxField" => ("3", 13, "11", None, None, None),
            // Option slot 10 holds the text colour on all 2 212 native
            // `PictureField` items: unset on the 2 195 without a `<TextColor>`,
            // readable on all 17 that carry one.
            "PictureField" => ("4", 24, "10", Some(10), None, Some(11)),
            // Likewise option slot 3 on all 1 381 native `RadioButtonField`
            // items.
            "RadioButtonField" => ("5", 12, "8", Some(3), None, None),
            "SpreadSheetDocumentField" => ("6", 32, "13", None, None, Some(15)),
            "TextDocumentField" => ("7", 16, "5", None, None, None),
            "CalendarField" => ("8", 24, "6", None, None, None),
            "GraphicalSchemaField" => ("14", 14, "3", None, None, None),
            "HTMLDocumentField" => ("15", 13, "3", None, None, Some(3)),
            "FormattedDocumentField" => ("17", 16, "1", None, None, Some(8)),
            // The PDF viewer field carries one member more than the other
            // field kinds at the same head offset: all five of UT
            // 11.5.27.75's `PDFDocumentField` items spell a 60-member
            // wrapper-`37` record with the name in slot 6, and none of them
            // holds a colour in any option slot, so no colour coordinate is
            // claimed here.
            "PDFDocumentField" => ("20", 14, "1", None, None, None),
            _ => return None,
        };
        let field_count_base = if item_tag == "PDFDocumentField" {
            60
        } else {
            59
        };
        // `top_level_offset == 1` was accepted for four kinds, then a fifth
        // (`RadioButtonField`), even though the offset itself
        // (`input_field_top_level_offset` at the one call site) is already
        // computed uniformly across every kind this schema serves -- the same
        // shift is already used to locate each kind's own discriminator and
        // options slot before a record ever reaches this guard. Rejecting the
        // schema here does not drop the item; each caller that depends on it
        // has its own unshifted-assuming fallback:
        //
        // - `parse_form_child_item_title`/`_tooltip` fall back to a
        //   positional guess (`&[9, 10]` / `&[10, 11]`) that assumes offset
        //   0. A shifted `RadioButtonField` had its title read correctly
        //   from slot 10 (the title fallback's second candidate, since slot
        //   9 is empty at this offset) while the tooltip fallback picked
        //   slot 10 too -- the same title text, read a second time -- instead
        //   of slot 11, the truly empty tooltip slot. Evidence: ERP UH
        //   MDM_Management `Catalogs/СправочникиБД/Forms/ФормаЭлемента`, item
        //   `СогласованиеСвязанныхОбъектов` -- wrapper `37`, 60 fields
        //   (offset 1), discriminator `5`, a 12-member option tuple headed
        //   `8` -- has its title at slot 10 and an empty `{1,0}` at slot 11;
        //   native writes `<Title>` and no `<ToolTip>` at all.
        // - `parse_form_schema_backed_child_item_events` has no fallback at
        //   all for `SpreadSheetDocumentField`: its only route to the
        //   field's own event collection is `options.get(schema
        //   .collection_slot())`, gated on this same schema matching. A
        //   rejected schema means every event on the field goes unwritten,
        //   `DetailProcessing` included, not just misread. Evidence: ERP UH
        //   MDM_Management `CommonForms/ИсторияСогласованияЦентрализованнаяБаза`,
        //   item `ИсторияСогласования` -- wrapper `37`, 60 fields (offset 1),
        //   discriminator `6`, a 32-member option tuple headed `13` (all
        //   exactly what `FormSpreadsheetDocumentFieldProperties` already
        //   requires of an unshifted `SpreadSheetDocumentField`) -- carries
        //   its event collection at option slot 18: `{1,
        //   2988b2a5-c887-4928-94ae-5d0c9c31e999 (the platform's
        //   `DetailProcessing` event id),
        //   "ИсторияСогласованияОбработкаРасшифровки", 1, 0,
        //   2988b2a5-c887-4928-94ae-5d0c9c31e999, 0, 1}` -- the exact shape
        //   `parse_form_schema_backed_event_record` already parses
        //   correctly for every *unshifted* `SpreadSheetDocumentField` (70
        //   native `DetailProcessing` occurrences across ssl/sslbase/ut/mdm/
        //   ws, none of them previously offset 1). Native writes `<Events>`
        //   with this one `DetailProcessing` entry.
        if wrapper != "37"
            || field_count != field_count_base + top_level_offset
            || top_level_offset > 1
            || (top_level_offset == 1
                && !matches!(
                    item_tag,
                    "LabelField"
                        | "InputField"
                        | "CheckBoxField"
                        | "PictureField"
                        | "RadioButtonField"
                        | "SpreadSheetDocumentField"
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
            html_document_options: item_tag == "HTMLDocumentField",
            picture_field_options: item_tag == "PictureField",
            title_slot: 9 + top_level_offset,
            // The footer's own caption sits ten slots past the title's, in the
            // slot that holds the localised-string container the platform fills
            // for `<FooterText>`.  Read off the field items of the native
            // "1С:Управление торговлей 11.5.27.75" form dumps: the slot holds a
            // populated container on every one of the 9 items the platform
            // writes a `<FooterText>` on -- one `LabelField`, seven
            // `InputField`, one `PictureField` -- and an empty one on every
            // field item that carries none.
            footer_text_slot: 19 + top_level_offset,
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
                    // Slot 14 reads `1` on the one `PDFDocumentField` of UT
                    // 11.5.27.75 the platform writes `<ReadOnly>true</ReadOnly>`
                    // on and `0` on the other four, which carry no element.
                    | "PDFDocumentField"
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
            // `Enabled` is not a property of four favoured field kinds: slot
            // `13 + top_level_offset` carries it on every kind this schema
            // admits. Read on the whole native UT 11.5.27.75 tree the slot is a
            // total function on each of them - `0` on exactly the items that
            // carry `<Enabled>false</Enabled>` and never on one that does not:
            // `RadioButtonField` 6 of 1 386, `SpreadSheetDocumentField` 1 of
            // 222, `HTMLDocumentField` 1 of 178, and no `0` at all on the 68
            // `TextDocumentField`, 41 `FormattedDocumentField`, 7
            // `CalendarField` and 2 `GraphicalSchemaField` items, which is why
            // restricting the slot to four kinds hid three writable items
            // instead of protecting anything.
            enabled_slot: Some(13 + top_level_offset),
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
            // The text document field keeps its width cap in its own option
            // tuple, not in the extended input-field tuple the shared reader
            // samples: over all 68 `TextDocumentField` records the export
            // walks, member 10 is `1` on the 65 without an `<AutoMaxWidth>`
            // and `0` on exactly the 3 that carry
            // `<AutoMaxWidth>false</AutoMaxWidth>`.
            auto_max_width_option_slot: (item_tag == "TextDocumentField").then_some(10),
            // `EqualItemsWidth` rides member 11 of the check box option tuple:
            // over all 5 954 `CheckBoxField` records with that tuple the member
            // is `2` on the 5 951 without the element and `1`/`0` on exactly
            // the 2 `true` and 1 `false` the platform writes.
            equal_items_width_option_slot: (item_tag == "CheckBoxField").then_some(11),
        })
    }

    pub(crate) const fn title_slot(self) -> usize {
        self.title_slot
    }

    pub(crate) const fn footer_text_slot(self) -> usize {
        self.footer_text_slot
    }

    /// `Zoomable` flag of the `PictureField` option tuple.  Over all 2 220
    /// `PictureField` option tuples the export walks, the slot holds `1` on
    /// exactly the 6 items the platform writes `<Zoomable>true</Zoomable>` on
    /// and `0` on the other 2 214.
    pub(crate) const fn picture_field_zoomable_option_slot(self) -> Option<usize> {
        if self.picture_field_options {
            Some(7)
        } else {
            None
        }
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

    pub(crate) fn auto_max_width(self, options: &[&str]) -> Option<bool> {
        (options.get(self.auto_max_width_option_slot?)?.trim() == "0").then_some(false)
    }

    pub(crate) fn equal_items_width(self, options: &[&str]) -> Option<bool> {
        match options.get(self.equal_items_width_option_slot?)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
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

    /// Slot `25 + top_level_offset` is the footer alignment of *every* field
    /// kind this schema admits, and it carries all three written codes, not
    /// only `Left`.
    ///
    /// Read over the whole native UT 11.5.27.75 tree -- 59 121 items across the
    /// eleven admitted kinds -- the slot is a total function of the platform's
    /// `<FooterHorizontalAlign>`: `0` on the 241 items that say `Left`, `1` on
    /// the 5 that say `Center`, `2` on the 14 that say `Right`, and `3` on all
    /// 58 861 that carry no element at all. No code maps to two answers and no
    /// kind disagrees with another: `InputField` 143/0/9, `LabelField` 71/2/0,
    /// `PictureField` 9/3/5, `CheckBoxField` 15/0/0, `RadioButtonField` 1/0/0
    /// and `SpreadSheetDocumentField` 2/0/0 written, with `CalendarField`,
    /// `TextDocumentField`, `FormattedDocumentField`, `GraphicalSchemaField`
    /// and `HTMLDocumentField` reading `3` throughout and writing nothing.
    /// Reading only code `0` is what hid the nineteen `Center`/`Right` items.
    pub(crate) fn footer_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        match fields.get(25 + self.top_level_offset)?.trim() {
            "0" => Some("Left"),
            "1" => Some("Center"),
            "2" => Some("Right"),
            _ => None,
        }
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
            .then(|| {
                FormSpreadsheetDocumentFieldProperties::from_raw_layout(
                    fields,
                    options,
                    self.top_level_offset,
                )
            })
            .flatten()
    }

    /// The member of the field's option tuple a slot names, or `None` when the
    /// tuple does not carry it.
    ///
    /// A tuple normalized up from a short revision holds
    /// [`FORM_ABSENT_MEMBER`] in every member that revision does not carry.
    /// That is an absence, not a value: a reader that got the placeholder
    /// through would report the property malformed rather than missing, and a
    /// malformed choice-parameter member is a hard writer refusal.
    pub(crate) fn input_field_option<'a>(
        self,
        options: &'a [&'a str],
        slot: FormInputFieldExtendedOptionSlot,
    ) -> Option<&'a str> {
        self.input_field_options
            .then(|| options.get(slot.index()).copied())
            .flatten()
            .filter(|member| *member != FORM_ABSENT_MEMBER)
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
pub(crate) struct FormButtonColorSchema {
    top_level_offset: usize,
}

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

    /// The bound slot a command button spells its command's parameter source
    /// in.
    ///
    /// The 53-member layout is the 52-member one with the conditional-appearance
    /// prefix inserted ahead of the name, which is what `top_level_offset`
    /// carries for `enabled`, `check`, `font`, `parameter`, the geometry pair
    /// and both stretch flags on this very schema. The slot used to be spelled
    /// for the unprefixed layout alone and answered nothing for the prefixed
    /// one, so a prefixed command button was written with no `<DataPath>` at
    /// all. ERP УХ 3.2.12.6
    /// `Catalogs/ГруппыВНАМСФО/Forms/ФормаЭлемента` button
    /// `ФормаОтчетДвиженияВНАДвиженияВНАМСФО` spells its name at member 6 and
    /// the chain `{2,{1},{-8}}` at member 10, one behind each of the
    /// unprefixed positions, and the platform writes
    /// `<DataPath>Объект.Ref</DataPath>` for it.
    pub(crate) const fn data_path_slot(self) -> Option<usize> {
        Some(9 + self.top_level_offset)
    }

    /// The metadata object a command button passes to its command.
    ///
    /// Slot `33 + top_level_offset` is a total function of the platform's
    /// `<Parameter>` over all 27 776 native `Button` items of UT 11.5.27.75:
    /// it holds the single member `"U"` on the 27 766 that carry no element,
    /// and the typed value `{"#", <type>, <value>}` on exactly the ten that do,
    /// the type being the metadata-object-reference type on all ten and the
    /// value dereferencing to the reference the platform writes, character for
    /// character. The property had no reader at all, so the writer had nothing
    /// to emit.
    pub(crate) const fn parameter_slot(self) -> usize {
        33 + self.top_level_offset
    }

    /// The check-mark state a command-bar button shows next to its title.
    ///
    /// Slot `24 + top_level_offset` is a total function of the platform's
    /// `<Check>` over all 27 779 native `Button` items of UT 11.5.27.75: it
    /// holds `1` on exactly the 47 items that carry `<Check>true</Check>` and
    /// `0` on every other item of the 52-slot layout, with no item holding `1`
    /// and no element, and none holding the element without the `1`. The
    /// 53-slot layout shifts the whole record one slot behind its
    /// conditional-appearance prefix, which is what `top_level_offset` already
    /// carries for every other member of this schema.
    ///
    /// The property had no reader at all, so the writer had nothing to emit.
    pub(crate) fn check(self, fields: &[&str]) -> Option<bool> {
        (fields.get(24 + self.top_level_offset)?.trim() == "1").then_some(true)
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
    /// The three colour members of a `Button`, in both of its record shapes.
    ///
    /// The 53-member record is the 52-member one with the
    /// conditional-appearance prefix ahead of the name; the schema used to
    /// admit only the unprefixed shape, so a prefixed button lost `<BackColor>`,
    /// `<TextColor>` and `<BorderColor>` outright rather than reading them one
    /// slot later.
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

    pub(crate) const fn back_color_slot(self) -> usize {
        19 + self.top_level_offset
    }

    pub(crate) const fn text_color_slot(self) -> usize {
        20 + self.top_level_offset
    }

    pub(crate) const fn border_color_slot(self) -> usize {
        21 + self.top_level_offset
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
            // Same prefixed/unprefixed pair as every other member of a
            // `Button`: the 53-member record carries the conditional-appearance
            // prefix ahead of the name and shifts the whole tail one slot.
            ("31", 52, "Button", 0) | ("31", 53, "Button", 1) => Some(Self {
                slot: 45 + top_level_offset,
            }),
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

/// `Shape` and `PictureLocation` of a `Button`, the two neighbours of the
/// shape-representation code in the same fixed tail.
///
/// UT 11.5.27.75, all 27 774 traced `Button` items (field counts 52 and 53):
/// reverse offset 8 is `0` on the 27 746 buttons without a `<Shape>` and `1`/`2`
/// on the 8 that say `Usual` and the 20 that say `Oval`; reverse offset 5 is `0`
/// on the 27 745 without a `<PictureLocation>` and `1`/`2` on the 17 that say
/// `Left` and the 12 that say `Right`.  Neither code ever maps to two different
/// platform answers, and no other code occurs.  The properties were read
/// nowhere before, so the writer had nothing to emit.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormButtonShapeSchema;

impl FormButtonShapeSchema {
    const SHAPE_REVERSE_OFFSET: usize = 8;
    const PICTURE_LOCATION_REVERSE_OFFSET: usize = 5;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
    ) -> Option<Self> {
        (wrapper == "31" && matches!(field_count, 52 | 53) && item_tag == "Button").then_some(Self)
    }

    pub(crate) fn shape(self, fields: &[&str]) -> Option<&'static str> {
        let slot = fields.len().checked_sub(Self::SHAPE_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "1" => Some("Usual"),
            "2" => Some("Oval"),
            _ => None,
        }
    }

    pub(crate) fn picture_location(self, fields: &[&str]) -> Option<&'static str> {
        let slot = fields
            .len()
            .checked_sub(Self::PICTURE_LOCATION_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "1" => Some("Left"),
            "2" => Some("Right"),
            _ => None,
        }
    }
}

/// Geometry and alignment of a `SearchStringAddition`.
///
/// The addition is a fixed 24-member `5`-wrapped record whose own option tuple
/// sits in slot 13 and always has 11 members.  UT 11.5.27.75, all 4 772 traced
/// `SearchStringAddition` items, every position below a total function of the
/// platform answer with no code mapping to two answers:
///
///   * option member 1 is `0` on the 4 755 additions without a `<Width>` and
///     the written width itself on all 17 that carry one;
///   * option member 2 is `2` on the 4 753 without a `<HorizontalStretch>` and
///     `0`/`1` on the 15 that say `false` and the 4 that say `true`;
///   * option member 8 is `1` on the 4 757 without an `<AutoMaxWidth>` and `0`
///     on all 15 that say `false`;
///   * option member 9 is `0` on the 4 765 without a `<MaxWidth>` and the
///     written width on all 7 that carry one;
///   * top-level slot 21 is `3` on the 4 765 without a
///     `<GroupHorizontalAlign>` and `0`/`2` on the 1 that says `Left` and the 6
///     that say `Right`;
///   * top-level slot 11 is `0` on the 4 766 without a
///     `<ToolTipRepresentation>` and `1`/`3` on the 3 that say `None` and the 3
///     that say `Button`.
///
/// None of the six had a reader, so the writer had nothing to emit.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormSearchStringAdditionSchema;

impl FormSearchStringAdditionSchema {
    pub(crate) const OPTIONS_SLOT: usize = 13;
    const FIELD_COUNT: usize = 24;
    const OPTION_COUNT: usize = 11;
    const WIDTH_OPTION_SLOT: usize = 1;
    const HORIZONTAL_STRETCH_OPTION_SLOT: usize = 2;
    const AUTO_MAX_WIDTH_OPTION_SLOT: usize = 8;
    const MAX_WIDTH_OPTION_SLOT: usize = 9;
    const GROUP_HORIZONTAL_ALIGN_SLOT: usize = 21;
    const TOOLTIP_REPRESENTATION_SLOT: usize = 11;

    pub(crate) fn from_raw_layout(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        options: &[&str],
    ) -> Option<Self> {
        (wrapper == "5"
            && field_count == Self::FIELD_COUNT
            && item_tag == "SearchStringAddition"
            && options.len() == Self::OPTION_COUNT)
            .then_some(Self)
    }

    fn dimension(self, options: &[&str], slot: usize) -> Option<String> {
        let value = options.get(slot)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_owned())
    }

    pub(crate) fn width(self, options: &[&str]) -> Option<String> {
        self.dimension(options, Self::WIDTH_OPTION_SLOT)
    }

    pub(crate) fn max_width(self, options: &[&str]) -> Option<String> {
        self.dimension(options, Self::MAX_WIDTH_OPTION_SLOT)
    }

    pub(crate) fn horizontal_stretch(self, options: &[&str]) -> Option<bool> {
        match options
            .get(Self::HORIZONTAL_STRETCH_OPTION_SLOT)
            .map(|field| field.trim())
        {
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => None,
        }
    }

    pub(crate) fn auto_max_width(self, options: &[&str]) -> Option<bool> {
        (options
            .get(Self::AUTO_MAX_WIDTH_OPTION_SLOT)
            .map(|field| field.trim())
            == Some("0"))
        .then_some(false)
    }

    pub(crate) fn group_horizontal_align(
        self,
        fields: &[&str],
    ) -> Option<FormFieldGroupHorizontalAlign> {
        FormFieldGroupHorizontalAlign::from_raw_value(
            fields.get(Self::GROUP_HORIZONTAL_ALIGN_SLOT)?.trim(),
        )
    }

    pub(crate) fn tooltip_representation(self, fields: &[&str]) -> Option<&'static str> {
        decode_form_tooltip_representation(fields.get(Self::TOOLTIP_REPRESENTATION_SLOT)?.trim())
    }

    pub(crate) fn properties(
        self,
        fields: &[&str],
        options: &[&str],
    ) -> FormSearchStringAdditionProperties {
        FormSearchStringAdditionProperties {
            width: self.width(options),
            max_width: self.max_width(options),
            horizontal_stretch: self.horizontal_stretch(options),
            auto_max_width: self.auto_max_width(options),
            group_horizontal_align: self
                .group_horizontal_align(fields)
                .map(FormFieldGroupHorizontalAlign::xml_value),
            tooltip_representation: self.tooltip_representation(fields),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FormSearchStringAdditionProperties {
    pub(crate) width: Option<String>,
    pub(crate) max_width: Option<String>,
    pub(crate) horizontal_stretch: Option<bool>,
    pub(crate) auto_max_width: Option<bool>,
    pub(crate) group_horizontal_align: Option<&'static str>,
    pub(crate) tooltip_representation: Option<&'static str>,
}

/// `ShapeRepresentation` of a `Popup`.
///
/// The code lives in member 6 of the popup's own nine-member option tuple, not
/// in a top-level slot; the property had no reader at all, so all 89 native
/// occurrences were lost.  UT 11.5.27.75, all 3 911 traced `Popup` items:
/// member 6 is `0` on the 3 822 popups that carry nothing, `2` on the 3 that
/// say `WhenActive` and `3` on the 86 that say `None`, with no code mapping to
/// two answers -- the same table `Button` uses for the property.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormPopupShapeRepresentationSchema;

impl FormPopupShapeRepresentationSchema {
    pub(crate) const OPTIONS_SLOT: usize = 20;
    const OPTION_COUNT: usize = 9;
    const SHAPE_REPRESENTATION_OPTION_SLOT: usize = 6;

    pub(crate) fn from_raw_layout(wrapper: &str, item_tag: &str, options: &[&str]) -> Option<Self> {
        (wrapper == "22"
            && item_tag == "Popup"
            && options.len() == Self::OPTION_COUNT
            && options.first().map(|field| field.trim()) == Some("7"))
        .then_some(Self)
    }

    pub(crate) fn shape_representation(self, options: &[&str]) -> Option<&'static str> {
        match options
            .get(Self::SHAPE_REPRESENTATION_OPTION_SLOT)
            .map(|field| field.trim())
        {
            Some("1") => Some("Always"),
            Some("2") => Some("WhenActive"),
            Some("3") => Some("None"),
            _ => None,
        }
    }
}

impl FormCheckBoxFieldSchema {
    const SHOW_IN_FOOTER_SLOT: usize = 21;
    const GROUP_HORIZONTAL_ALIGN_SLOT: usize = 53;
    const GROUP_VERTICAL_ALIGN_SLOT: usize = 54;
    const THREE_STATE_OPTION_SLOT: usize = 1;
    /// `EditFormat` slot of the 13-member `11`-discriminated option tuple.
    ///
    /// The check box spells its two captions the same way an input field spells
    /// a picture format, as an ordinary localized-string tuple. Over all 6 286
    /// `CheckBoxField` option tuples of the native UT 11.5.27.75 form bodies the
    /// slot holds the empty tuple `{1,0}` on 6 222, none of whose items carries
    /// an `<EditFormat>`, and a non-empty tuple on exactly the 64 items the
    /// platform writes one on, with the decoded text equal to the platform's own
    /// `<v8:content>` on every one of them.
    const EDIT_FORMAT_OPTION_SLOT: usize = 5;

    pub(crate) fn top_level_offset_for_raw_layout(
        wrapper: &str,
        field_count: usize,
    ) -> Option<usize> {
        match (wrapper, field_count) {
            ("37", 59) => Some(0),
            ("37", 60) => Some(1),
            // A `Table`'s own implicit `CheckBoxField` column (wrapper `35`,
            // see `form_child_item_tag`) reaches this schema with its own
            // conditional `UserVisible`-common prefix already stripped by
            // the caller (`parse_form_child_item_with_metadata_owners`'s
            // `wrapper35_prefix_slot` normalization), unlike wrapper `37`
            // whose own shift this schema still has to read itself. ERP УХ
            // MDM_Management's `InformationRegisters/СоответствиеЗаявокНаИзменениеНСИ/Forms/ФормаСписка`
            // carries its `ОбменВыполнен` `CheckBoxField` at 57 members
            // after that normalization, with the name already unshifted at
            // slot 6 exactly like wrapper `37`'s own offset-`0` shape --
            // hence offset `0` here too, not a new value.
            ("35", 57) => Some(0),
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
        if direct_discriminator != Some("3") {
            return None;
        }
        // The option block declares its own revision in its leading member, and
        // that revision names which slot holds `CheckBoxType`.
        //
        // Revision `11` at 13 members is the block every corpus writes under
        // form-body container revision `4`, and slot 12 is its code.  ERP УХ
        // 3.2.12.6's container-revision-`3` form bodies write revision `10` at
        // 12 members instead -- one member shorter, the leading member one
        // lower, `len - lead` the same 2 -- and the missing member is the last
        // one: over the whole block the two revisions agree member for member
        // on slots 0..11, both in kind and in value distribution, and `11` adds
        // slot 12 on the end.
        //
        // The code revision `10` reads is slot 4, the three-valued predecessor
        // `11` still mirrors.  On all 887 revision-`11` blocks of the БСП base
        // tree, slot 4 equals slot 12 wherever slot 12 is `0`/`1`/`2` and reads
        // `0` on the eight where slot 12 is `3` -- `Switcher`, the ordinal
        // revision `11` added.  On all 18 revision-`10` blocks the stand
        // carries, the platform's own `<CheckBoxType>` is `Auto` on the sixteen
        // whose slot 4 is `0`, `CheckBox` on the one that reads `1` and
        // `Tumbler` on the one that reads `2`, with no counter-example.  Slot 4
        // is not read under revision `11`, where slot 12 is authoritative and
        // already proven byte-for-byte across the corpus.
        let check_box_type_option_slot =
            match (options.first().map(|field| field.trim()), options.len()) {
                (Some("11"), 13) => 12,
                (Some("10"), 12) => 4,
                _ => return None,
            };
        Some(Self {
            top_level_offset,
            check_box_type_option_slot,
        })
    }

    pub(crate) const fn options_slot(self) -> usize {
        39 + self.top_level_offset
    }

    pub(crate) const fn edit_format_option_slot(self) -> usize {
        Self::EDIT_FORMAT_OPTION_SLOT
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

    // 6 210 traced check boxes: both slots are total functions under the shared
    // alignment tables (47 and 22 present).  The transcribed pair had omitted
    // `Center` from the horizontal table and `Bottom` from the vertical one,
    // which dropped 4 of the 22.
    pub(crate) fn group_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        form_group_horizontal_align_xml(
            fields.get(Self::GROUP_HORIZONTAL_ALIGN_SLOT + self.top_level_offset)?,
        )
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        form_group_vertical_align_xml(
            fields.get(Self::GROUP_VERTICAL_ALIGN_SLOT + self.top_level_offset)?,
        )
    }

    pub(crate) fn check_box_type(self, options: &[&str]) -> Option<&'static str> {
        match (
            options.get(1).map(|field| field.trim()),
            options
                .get(self.check_box_type_option_slot)
                .map(|field| field.trim()),
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
    /// Validates the conditional-`UserVisible` prefix shape and returns the
    /// value it actually carries.
    ///
    /// The census that named this slot only ever saw `Some(false)` -- the
    /// non-default state the platform writes
    /// `<UserVisible><xr:Common>false</xr:Common></UserVisible>` for -- on
    /// `Button` and `PictureField`. ERP UH MDM_Management's `LabelField` and
    /// `InputField` items carry the identical marker-and-tuple pair at the
    /// identical slots of the same wrapper-`37` layout, at the same
    /// evidenced field count and offset, but at the *default* value `true`,
    /// which the platform writes no `<UserVisible>` element for at all. The
    /// shape is the same envelope regardless of which field kind wraps it or
    /// which of the two values it holds, so both are read through here
    /// instead of only the one value the original two tags happened to show,
    /// and the caller gets the value itself rather than a presence marker.
    ///
    /// The payload is whatever the caller read out of the prefix tuple, not a
    /// bare flag: the envelope is the platform's rights tuple
    /// `{0,{0,{"B",<common>},<n>,<role uuid>,{"B",<value>},…}}`, whose
    /// declared member `<n>` may name roles that override the common answer.
    /// This function validates the record shape that puts a tuple at that
    /// slot and passes the caller's reading of it straight through.
    pub(crate) fn from_raw_layout<T>(
        wrapper: &str,
        field_count: usize,
        item_tag: &str,
        top_level_offset: usize,
        conditional_marker: Option<&str>,
        user_visible: Option<T>,
    ) -> Option<T> {
        match (
            wrapper,
            field_count,
            item_tag,
            top_level_offset,
            conditional_marker,
        ) {
            ("31", 53, "Button", 1, Some("1"))
            | (
                "37",
                60,
                "PictureField" | "LabelField" | "InputField" | "CheckBoxField" | "RadioButtonField"
                | "TextDocumentField",
                1,
                Some("1"),
            ) => user_visible,
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
            // `Page` (discriminator `4`) carries the same conditional
            // `UserVisible` prefix as the other grouping controls.  UT
            // 11.5.27.75: of the wrapper-`22` items the reader used to drop
            // whole, all 23 carry the prefix tuple `{0,{0,{"B",0},0}}` in slot
            // 5 and discriminator `4` behind it, at field counts 33, 35 and 37
            // -- the same `31 + 2k` progression the accepted discriminators
            // run on.  Each of the 23 is a `Page` the platform writes with
            // `<UserVisible><xr:Common>false</xr:Common></UserVisible>`, so the
            // whole page and its subtree were being lost to the discriminator
            // whitelist alone.
            //
            // `Some(true)` -- the default state the platform writes no
            // `<UserVisible>` for -- belongs here too, the same way it
            // already does for the `8`/`9` arm below: ERP УХ
            // MDM_Management's `Catalogs/ВнешниеИнформационныеБазы/Forms/ФормаЭлемента`
            // carries four root `UsualGroup` items (discriminator `5`,
            // field counts 31/33/35/35) with the tuple's own flag reading
            // `1`, and the platform writes none of their four with a
            // `<UserVisible>` element. Requiring `false` dropped all four
            // groups, and with them the `LabelDecoration` sibling below and
            // the whole subtree each carried.
            //
            // `CommandBar` (`0`), `Popup` (`1`) and `ButtonGroup` (`6`) carry
            // the identical prefix tuple in the identical slot, on the
            // identical `31 + 2k` progression -- confirmed directly on real
            // ERP УХ 3.2.12.6 bytes at field count 35 (`k = 2`) for all
            // three: `CommandBar` `ГруппаКоманднаяПанель`
            // (`Documents/ВерсияДокументацииЗакупочныхПроцедур/Forms/ФормаРедактированияТекстаЗакупочнойПроцедуры`),
            // `Popup` `ИзменитьДолюУчастия`
            // (`DocumentJournals/ДвижениеИнвестиций/Forms/ФормаРеестраИнвестиций`)
            // and `ButtonGroup` `ФормаГруппаВерсии`
            // (`DataProcessors/ГрафикФИМСФО/Forms/ГрафикФИ`). Before this,
            // `form_child_item_tag` read the shifted-discriminator slot
            // without the shift (landing on the prefix tuple's own opening
            // brace, never `"0"`..`"9"`) and `form_child_item_tag`/
            // `parse_form_child_item_name` both refused, so the whole item
            // -- and, when it was a container, everything nested inside it
            // -- was dropped silently rather than just the `<UserVisible>`
            // property this schema exists to read (doctrine point 2/6).
            // `FormChildItemVisibleSchema` below already lists all seven
            // wrapper-`22` kinds together for the very same prefix tuple;
            // this arm had only ever grown the four grouping kinds.
            (
                "22",
                field_count,
                Some(false) | Some(true),
                Some("0" | "1" | "2" | "3" | "4" | "5" | "6"),
            ) if field_count >= 31 && (field_count - 31) % 2 == 0 => Some(Self { prefix_slot: 5 }),
            // A `Table`'s own service `ContextMenu` (discriminator `8`) and
            // `AutoCommandBar` (discriminator `9`) carry the identical
            // marker-and-tuple prefix at the identical slot, but at their own
            // much shorter 30-member base layout rather than the 31+2k one
            // `Page`/group items run on, and ERP UH MDM_Management's `Список`
            // dynamic-list tables carry it at the *default* value `true` --
            // the state the platform writes no `<UserVisible>` for -- rather
            // than the only value the `Page` census saw. Without this arm the
            // reader lost the `ContextMenu`/`AutoCommandBar` item whole.
            // The 30-member base layout above is itself the *childless*
            // shape: a root `AutoCommandBar` that carries its own
            // `<ChildItems>` (e.g. ERP УХ MDM_Management's
            // `Catalogs/ВнешниеИнформационныеБазы/Forms/ФормаЭлемента`, whose
            // root command bar holds one `<Button name="Справка">`) appends
            // the same trailing `count,(uuid,value)*count` pairs the
            // `Page`/group items already run 31+2k on, so admitting the same
            // `+2k` progression here rather than the bare `30` reproduces it
            // instead of losing the whole `<AutoCommandBar>` a second time.
            ("22", field_count, Some(false) | Some(true), Some("8" | "9"))
                if field_count >= 30 && (field_count - 30) % 2 == 0 =>
            {
                Some(Self { prefix_slot: 5 })
            }
            // A decoration takes the very same prefix in the very same slot.
            // UT 11.5.27.75 spells exactly one: a 37-member wrapper-`12`
            // record -- the 36-member decoration layout plus the tuple
            // `{0,{0,{"B",0},0}}` at slot 5, with the decoration discriminator
            // behind it at slot 6 -- and the platform writes that item as a
            // `LabelDecoration` carrying
            // `<UserVisible><xr:Common>false</xr:Common></UserVisible>`.  It
            // was dropped whole, subtree and all, because only wrapper `22`
            // was ever admitted here.  The normalized record is re-checked by
            // `FormDecorationHeaderSchema`, so this arm only has to name the
            // one member the prefix adds.
            //
            // `Some(true)` admits the default (no `<UserVisible>` written)
            // state here too: ERP УХ MDM_Management's root `LabelDecoration`
            // `ДекорацияНСИ` (`Catalogs/ВнешниеИнформационныеБазы/Forms/ФормаЭлемента`
            // and its siblings) carries the identical 37-member layout and
            // discriminator `0` with the tuple's flag reading `1`, and the
            // platform writes no `<UserVisible>` for it.
            ("12", 37, Some(false) | Some(true), Some("0" | "1")) => Some(Self { prefix_slot: 5 }),
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
            // `Some(false)` is the non-default state the platform writes
            // `<UserVisible><xr:Common>false</xr:Common></UserVisible>` for.
            // `Some(true)` is the default state, written as no `<UserVisible>`
            // element at all -- ERP UH MDM_Management's `Список` dynamic-list
            // tables carry the very same prefix tuple at its default value, so
            // the slot has to be recognized on both values or a default-valued
            // table is misread past this point entirely.
            ("55", field_count, Some(false) | Some(true), Some("1"))
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
        button_top_level_offset: usize,
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
            // The conditional `UserVisible` prefix takes the button's name slot
            // and pushes every later member of the record along by one, so a
            // shifted button is a 53-member record whose `Visible` code sits at
            // 27, not 26.  Spelling the length and the slot at offset 0 alone
            // declined the shifted record outright and the element went
            // unwritten.
            //
            // Census over both configurations whose form layouts were dumped,
            // pairing each `31` record with the native `<Button>` of the same
            // name: ERP УХ 3.2.12.6 has 80 430 unshifted buttons and 4 916
            // shifted ones, Документооборот КОРП 3.0.21.3 15 076 and 91.  Slot
            // `26 + offset` is a total function of the native spelling in every
            // one of the four groups -- `0` on all 5 395 + 166 + 524 + 0 that
            // carry `<Visible>false</Visible>` and `1` on all the rest -- with
            // no third code and no counter-example.
            ("31", "Button", _) if field_count == 52 + button_top_level_offset => {
                26 + button_top_level_offset
            }
            // Preserve the three wrapper-48 field owners decoded by the legacy path.
            ("48", "LabelField", Some("1"))
            | ("48", "InputField", Some("2"))
            | ("48", "CheckBoxField", Some("3"))
                if field_count > 20 =>
            {
                43 + top_level_offset
            }
            // The five document-shaped field kinds share the wrapper-37 layout
            // and the same visibility slot as the six kinds already listed.
            // Over the 518 `SpreadSheetDocumentField`, `TextDocumentField`,
            // `FormattedDocumentField`, `HTMLDocumentField`, `CalendarField`
            // and `GraphicalSchemaField` items of the UT 11.5.27.75 native tree
            // slot 43 reads `0` on exactly the 10 whose native document carries
            // `<Visible>false</Visible>` and `1` on the other 508, with no
            // third code -- the same total function the listed kinds show.
            ("37", "LabelField", Some("1"))
            | ("37", "InputField", Some("2"))
            | ("37", "CheckBoxField", Some("3"))
            | ("37", "PictureField", Some("4"))
            | ("37", "RadioButtonField", Some("5"))
            | ("37", "SpreadSheetDocumentField", Some("6"))
            | ("37", "TextDocumentField", Some("7"))
            | ("37", "CalendarField", Some("8"))
            | ("37", "GraphicalSchemaField", Some("14"))
            | ("37", "HTMLDocumentField", Some("15"))
            | ("37", "FormattedDocumentField", Some("17"))
                if matches!((field_count, top_level_offset), (59, 0) | (60, 1)) =>
            {
                43 + top_level_offset
            }
            // The three special-field kinds share that very layout: all 56
            // `ProgressBarField`, `TrackBarField` and `ChartField` items of
            // UT 11.5.27.75 are wrapper-`37` 59-member records, and slot 43
            // reads `1` on the 55 whose native document carries no
            // `<Visible>`, and `0` on the one chart field it writes
            // `<Visible>false</Visible>` on. They were simply never listed.
            ("37", "ProgressBarField", Some("9"))
            | ("37", "TrackBarField", Some("10"))
            | ("37", "ChartField", Some("11"))
                if field_count == 59 && top_level_offset == 0 =>
            {
                43
            }
            // The three list additions share one 24-member wrapper-`5` layout
            // and keep the flag in slot 9. All 13 942 of them in UT
            // 11.5.27.75 -- 4 773 search strings, 4 543 view statuses and
            // 4 626 search controls -- read `1` there except the single
            // search string the platform writes `<Visible>false</Visible>`
            // on, which reads `0`. No third code occurs.
            ("5", "SearchStringAddition", Some("0"))
            | ("5", "ViewStatusAddition", Some("1"))
            | ("5", "SearchControlAddition", Some("2"))
                if field_count == 24 =>
            {
                9
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
            // The two alignment slots are enumerations with four ordinals, so the
            // layout test admits all four.  Admitting only the values that had
            // been seen (`2|3` and `1|2|3`) rejected 15 of the 1 840 native
            // command bars outright - and rejecting the item discards every
            // property this schema reads, not just the alignment: 14 of those 15
            // carry `GroupHorizontalAlign` and 8 carry `GroupVerticalAlign`.
            || !matches!(
                fields
                    .get(
                        fields
                            .len()
                            .checked_sub(Self::GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET)?
                    )
                    .map(|field| field.trim()),
                Some("0" | "1" | "2" | "3")
            )
            || !matches!(
                fields
                    .get(
                        fields
                            .len()
                            .checked_sub(Self::GROUP_VERTICAL_ALIGN_REVERSE_OFFSET)?
                    )
                    .map(|field| field.trim()),
                Some("0" | "1" | "2" | "3")
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

    // Reverse slots 3 and 2 are total functions on all 1 840 traced command bars
    // under the shared alignment tables (111 and 29 present).  The transcribed
    // pair had omitted `Left` and `Top`, losing 11 and 4 respectively.
    pub(crate) fn group_horizontal_align(self, fields: &[&str]) -> Option<&'static str> {
        form_group_horizontal_align_xml(
            fields.get(
                fields
                    .len()
                    .checked_sub(Self::GROUP_HORIZONTAL_ALIGN_REVERSE_OFFSET)?,
            )?,
        )
    }

    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        form_group_vertical_align_xml(
            fields.get(
                fields
                    .len()
                    .checked_sub(Self::GROUP_VERTICAL_ALIGN_REVERSE_OFFSET)?,
            )?,
        )
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
    hidden_state_title_back_color_option_slot: Option<usize>,
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
            // A `Popup` keeps the same flag in the same slot: over all 3 911
            // native popups of UT 11.5.27.75 slot 11 reads `1` on exactly the
            // one whose document carries `<ReadOnly>true</ReadOnly>` and `0` on
            // the other 3 910, with no third code.
            ("Popup", Some("1"), 9, Some("7")) => Some(Self),
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
                // Unobserved on a `Page`: no native page carries a
                // `HiddenStateTitleBackColor`, so this layout refuses rather
                // than borrowing the usual group's coordinate.
                hidden_state_title_back_color_option_slot: None,
            });
        }
        if wrapper != "22" || field_count < 30 || (field_count - 30) % 2 != 0 {
            return None;
        }
        // The usual group's own 29-member option tuple carries a second
        // colour beside `BackColor`: option 23 is its
        // `HiddenStateTitleBackColor`.  Census of every colour-bearing item
        // record of UT 11.5.27.75 -- all 11 099, scanned at every top-level
        // and nested slot -- finds a colour there on exactly one item, and it
        // is exactly the one the platform writes the element on.
        let (option_slot, back_color_option_slot, hidden_state_title_back_color_option_slot) =
            match (
                item_tag,
                direct_discriminator,
                options.len(),
                options.first().map(|field| field.trim()),
            ) {
                ("ColumnGroup", Some("2"), 12, Some("2")) => (2, None, None),
                ("UsualGroup", Some("5"), 29, Some("29")) => (4, Some(9), Some(23)),
                // The compact/legacy 28-member bag keeps `ShowTitle` at the
                // same slot 4 the wide bag uses; see
                // `parse_form_usual_group_extended_options`'s `"28"` arm for
                // the evidence (five native records, two configurations).
                // No colour has been observed at any slot of this bag, so
                // neither colour coordinate is claimed.
                ("UsualGroup", Some("5"), 28, Some("28")) => (4, None, None),
                _ => return None,
            };
        Some(Self {
            option_slot,
            back_color_option_slot,
            hidden_state_title_back_color_option_slot,
        })
    }

    pub(crate) fn show_title(self, options: &[&str]) -> Option<bool> {
        (options.get(self.option_slot)?.trim() == "0").then_some(false)
    }

    pub(crate) const fn back_color_option_slot(self) -> Option<usize> {
        self.back_color_option_slot
    }

    pub(crate) const fn hidden_state_title_back_color_option_slot(self) -> Option<usize> {
        self.hidden_state_title_back_color_option_slot
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

/// The form's own `Enabled` sits alone in root field 15 of the `50` layout,
/// immediately behind the `Customizable` field.
///
/// UT 11.5.27.75 native tree, all 5 201 `Form.xml` roots (every one of them a
/// `50` layout): field 15 reads `0` for exactly the three roots whose native
/// document carries `<Enabled>false</Enabled>` and `1` for the other 5 198 that
/// omit it - a total function with no third code and no counter-example.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootEnabledSchema {
    slot: usize,
}

impl FormRootEnabledSchema {
    const SLOT: usize = 15;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        field_count: usize,
    ) -> Option<Self> {
        (root_discriminator == Some("50") && field_count > Self::SLOT)
            .then_some(Self { slot: Self::SLOT })
    }

    pub(crate) fn enabled(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.slot)?.trim() {
            "0" => Some(false),
            _ => None,
        }
    }
}

/// `VariantAppearance` is property-bag key 20: the same `{"#", uuid, {1, {id},
/// ""}}` reference to one of the form's own attributes that `ReportResult`
/// (key 5) and `DetailsData` (key 6) carry.
///
/// UT 11.5.27.75 native tree, all 5 201 roots: reading key 20 through that
/// shape resolves to exactly the four attribute names the native documents
/// write in `<VariantAppearance>` and to nothing on the other 5 197, the same
/// score the two neighbouring keys post (25/25 and 14/14, no false positive).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootVariantAppearanceSchema;

impl FormRootVariantAppearanceSchema {
    pub(crate) const PROPERTY_BAG_KEY: &'static str = "20";

    pub(crate) fn from_raw_layout(root_discriminator: Option<&str>) -> Option<Self> {
        (root_discriminator == Some("50")).then_some(Self)
    }
}

/// `SettingsStorage` sits alone in root field 8: the uuid of the metadata
/// object the form saves its user settings into, or the nil uuid when it saves
/// them where it saves them by default.
///
/// The field is not a bag entry, which is why the bag-keyed readers never found
/// it: it sits between the `AutoSaveDataInSettings` flag (field 7) and the
/// title (field 10), ahead of the bag's own count in field 18.
///
/// Evidence: the eight stand corpora, all 22 632 native `Form.xml` roots read
/// through the export's own root census. Field 8 holds a non-nil uuid on
/// exactly the 20 roots whose native document carries `<SettingsStorage>` and
/// the nil uuid on the other 22 612 - a total function with no counter-example
/// in either direction, over both the `49` and the `50` discriminator. The
/// uuid names either a settings-storage object (`SettingsStorage.Общие` on 13
/// roots, `SettingsStorage.ХранилищеВариантовОтчетов` on 3) or a form
/// (`Report.<…>.Form.<…>` on 4, each of them the naming form itself), so it is
/// read through the reference index that carries both.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootSettingsStorageSchema {
    slot: usize,
}

impl FormRootSettingsStorageSchema {
    const SLOT: usize = 8;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        field_count: usize,
    ) -> Option<Self> {
        (matches!(root_discriminator, Some("49") | Some("50")) && field_count > Self::SLOT)
            .then_some(Self { slot: Self::SLOT })
    }

    pub(crate) const fn slot(self) -> usize {
        self.slot
    }
}

/// A form-root property that names one of the form's own items: an `{"N", id}`
/// property-bag value, with `0` standing for "no item".
///
/// Two properties are written in exactly this shape and are read through one
/// schema rather than two, so the pair cannot drift apart the way this series
/// has watched twinned tables drift four times already:
///
///   * `CustomSettingsFolder`, property-bag key 23;
///   * `GroupList`, property-bag key 1.
///
/// Evidence, the eight stand corpora (22 632 native `Form.xml` roots read
/// through the export's own root census):
///
///   * key 23 reads `{"N",0}` on every root whose native document omits
///     `<CustomSettingsFolder>` (169 of them) and a non-zero id on every root
///     that carries it (30), and no root carries the property without the key.
///     ERP УХ contributes 16 of the 30 on a `49` root, which is why the
///     discriminator gate below admits `49` beside `50`: gating on `50` alone
///     dropped every one of them;
///   * key 1 reads `{"N",0}` or nothing at all on all 22 620 roots whose
///     native document omits `<GroupList>` and a non-zero id on all 12 that
///     carry it, again across both discriminators.
///
/// An id the form's own item table does not know is not a refusal: the platform
/// writes the dangling `<id>:<form-item class uuid>` spelling instead, the same
/// spelling it uses for a picture or a command it cannot name. Three ERP УХ
/// roots spell `<CustomSettingsFolder>3:02023637-…` and two spell
/// `<GroupList>5:02023637-…`, at ids that no item of those forms declares.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootItemReferenceSchema {
    property_bag_key: &'static str,
}

impl FormRootItemReferenceSchema {
    pub(crate) const CUSTOM_SETTINGS_FOLDER_KEY: &'static str = "23";
    pub(crate) const GROUP_LIST_KEY: &'static str = "1";

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        property_bag_key: &'static str,
    ) -> Option<Self> {
        matches!(root_discriminator, Some("49") | Some("50")).then_some(Self { property_bag_key })
    }

    pub(crate) const fn property_bag_key(self) -> &'static str {
        self.property_bag_key
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

/// How many optional blocks the root `50` trailer declares in its own member 2.
///
/// Every start-anchored slot in the trailer from member 3 onwards sits that
/// many members further out, because the declared blocks are inserted right
/// there. БСП, БСП демо, УТ and WMS declare `0` and end in the classic
/// 24-member trailer; ERP УХ and its MDM_Management declare `1`, carry one
/// `{22,...}` block, and run to 25:
///
/// ```text
/// count 0:  "" | "" | 0 |              1 | "" | 0 0 0 0 0 0 | 3 3 0 ...
/// count 1:  "" | "" | 1 | {22,{0},0,0,0,} | 1 | "" | 0 0 0 0 0 0 | 3 3 0 ...
///                     ^         block(s)    ^
///                   count                 member 3
/// ```
///
/// Over all 18 634 root `50` forms the export walks on the stand the declared
/// count equals `trailer.len() - 24` without a single exception, so the length
/// is used here to verify the count rather than to guess it: a trailer whose
/// two disagree is refused outright, and every reader that adds this offset to
/// its own base slot is then reading a position the blob itself declared.
///
/// Root `49` shares the layout exactly, minus root `50`'s one trailing member,
/// so its trailer runs to `23 + count` and its slots are the same `base +
/// count`. Its 1 548 forms (ERP УХ 1 543, MDM_Management 5) all declare a
/// count of `1`, all validate at 24 members and never at 23 or 25, and
/// reproduce root `50`'s value tables property for property with no
/// contradiction.
///
/// The block the count introduces is the form root's built-in
/// Navigator/quick-search child item, which is why the shift was first
/// described as a "Navigator gap". Every per-property confirmation gathered
/// under that name is a confirmation of this count, each on real ERP УХ
/// 3.2.12.6 bytes:
///
/// | property | form | native | wrong slot | right slot |
/// |---|---|---|---|---|
/// | `VerticalSpacing` | `Catalogs/ЗаявлениеОНазначенииПенсии/Forms/ФормаЭлемента` | `Half` (`2`) | 10 reads `0` | 11 reads `2` |
/// | `ConversationsRepresentation` | `Catalogs/ВариантыОтчетов/Forms/ФормаЭлемента` | `Show` (`1`) | 19 reads `0` | 20 reads `1` |
/// | `VerticalAlign` | `Catalogs/ВидыКонтактнойИнформации/Forms/ИсправлениеВидовКонтактнойИнформации` | `Bottom` (`2`) | 12, no count-list at all | 13 reads `2` |
/// | `SaveWindowSettings` | `Catalogs/КартыИсследованияТоваров/Forms/ФормаСписка` | `false` (`0`) | 23, no count-list at all | 24 reads `0` |
///
/// The shift is not one property's alone, and it was reached from two
/// directions: `FormRootVerticalScrollSchema` (slots 5/15 -> 6/16) and
/// `extract_form_show_title` (slot 17 -> 18) each arrived at the same
/// one-member offset independently, from ERP УХ MDM_Management, before the
/// count behind it was identified.
pub(crate) fn form_root_trailer_optional_blocks(
    root_discriminator: Option<&str>,
    trailer: &[&str],
) -> Option<usize> {
    const OPTIONAL_BLOCK_COUNT_SLOT: usize = 2;

    let base_trailer_fields = match root_discriminator {
        Some("50") => 24usize,
        Some("49") => 23,
        _ => return None,
    };
    let declared = trailer
        .get(OPTIONAL_BLOCK_COUNT_SLOT)?
        .trim()
        .parse::<usize>()
        .ok()?;
    (trailer.len() == base_trailer_fields.checked_add(declared)?).then_some(declared)
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
        trailer: &[&str],
    ) -> Option<Self> {
        let blocks = form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        Some(Self {
            trailer_slot: Self::TRAILER_SLOT.checked_add(blocks)?,
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

/// The form root's own grouping properties: the spacing pair and
/// `CollapseItemsByImportanceVariant` in the 24-slot trailer, and
/// `ChildItemsWidth` in root field 12.
///
/// UT 11.5.27.75 native tree, 5 184 traced roots keyed by output path: trailer
/// slot 9 is a total function for `HorizontalSpacing` (24 present), trailer slot
/// 10 for `VerticalSpacing` (33), trailer slot 20 for
/// `CollapseItemsByImportanceVariant` (27, `1->Use 2->DontUse`, `0` absent) and
/// root field 12 for `ChildItemsWidth` (14).  No counter-example on any of the
/// four; the spacing pair and the width read the same shared tables as
/// `UsualGroup` and `Page`.  The root carried none of the four before.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootGroupingSchema {
    horizontal_spacing_trailer_slot: usize,
    vertical_spacing_trailer_slot: usize,
    collapse_items_by_importance_trailer_slot: usize,
}

impl FormRootGroupingSchema {
    const CHILD_ITEMS_WIDTH_SLOT: usize = 12;

    const HORIZONTAL_SPACING_SLOT: usize = 9;
    const VERTICAL_SPACING_SLOT: usize = 10;
    const COLLAPSE_ITEMS_BY_IMPORTANCE_SLOT: usize = 20;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer: &[&str],
    ) -> Option<Self> {
        let blocks = form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        Some(Self {
            horizontal_spacing_trailer_slot: Self::HORIZONTAL_SPACING_SLOT.checked_add(blocks)?,
            vertical_spacing_trailer_slot: Self::VERTICAL_SPACING_SLOT.checked_add(blocks)?,
            collapse_items_by_importance_trailer_slot: Self::COLLAPSE_ITEMS_BY_IMPORTANCE_SLOT
                .checked_add(blocks)?,
        })
    }

    pub(crate) fn horizontal_spacing(self, trailer: &[&str]) -> Option<&'static str> {
        form_item_spacing_xml(trailer.get(self.horizontal_spacing_trailer_slot)?)
    }

    pub(crate) fn vertical_spacing(self, trailer: &[&str]) -> Option<&'static str> {
        form_item_spacing_xml(trailer.get(self.vertical_spacing_trailer_slot)?)
    }

    pub(crate) fn collapse_items_by_importance_variant(
        self,
        trailer: &[&str],
    ) -> Option<&'static str> {
        match trailer
            .get(self.collapse_items_by_importance_trailer_slot)?
            .trim()
        {
            "1" => Some("Use"),
            "2" => Some("DontUse"),
            _ => None,
        }
    }

    /// The width lives in the fixed root header, not in the trailer, so it is
    /// read from the root field array directly.
    pub(crate) fn child_items_width(
        root_discriminator: Option<&str>,
        fields: &[&str],
    ) -> Option<&'static str> {
        if root_discriminator != Some("50") {
            return None;
        }
        form_children_width_xml(fields.get(Self::CHILD_ITEMS_WIDTH_SLOT)?)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootVerticalScrollSchema {
    qualifier_slot: usize,
    mode_slot: usize,
}

impl FormRootVerticalScrollSchema {
    /// The qualifier/mode pair sits at trailer slots 5 and 15 plus whatever
    /// count the trailer declares in its own member 2.
    ///
    /// This started as three separate cases -- `(50, 24)` at 5/15, `(49, 24)`
    /// and `(49|50, 25)` at 6/16 -- each attributed one corpus at a time. They
    /// are the same rule: root `50` with count 0 runs to 24 members and reads
    /// 5/15; root `49` with count 1 also runs to 24 (it lacks root `50`'s
    /// trailing member) and root `50` with count 1 runs to 25, and both read
    /// 6/16. `form_root_trailer_optional_blocks` recovers the count, so the
    /// offsets follow from it rather than from a table of shapes.
    const QUALIFIER_SLOT: usize = 5;
    const MODE_SLOT: usize = 15;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer: &[&str],
    ) -> Option<Self> {
        let blocks = form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        Some(Self {
            qualifier_slot: Self::QUALIFIER_SLOT.checked_add(blocks)?,
            mode_slot: Self::MODE_SLOT.checked_add(blocks)?,
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
    Top,
    Center,
    Bottom,
}

impl FormRootVerticalAlign {
    /// The form root runs the same four-code alignment table its grouping
    /// controls do -- `0` `Top`, `1` `Center`, `2` `Bottom`, `3` nothing.
    /// Measured over all 5 139 form roots the export walks in UT 11.5.27.75:
    /// the trailer member is `3` on the 5 132 forms without a `<VerticalAlign>`
    /// and `0`/`1`/`2` on exactly the 2/2/3 that say `Top`, `Center` and
    /// `Bottom`, with no counter-example.  Only `Bottom` used to be decoded.
    pub(crate) fn from_raw_value(value: &str) -> Option<Self> {
        match value.trim() {
            "0" => Some(Self::Top),
            "1" => Some(Self::Center),
            "2" => Some(Self::Bottom),
            _ => None,
        }
    }

    pub(crate) fn from_xml_value(value: &str) -> Option<Self> {
        match value {
            "Top" => Some(Self::Top),
            "Center" => Some(Self::Center),
            "Bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    pub(crate) const fn raw_value(self) -> &'static str {
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
pub(crate) struct FormRootVerticalAlignSchema {
    trailer_slot: usize,
    children_align_trailer_slot: usize,
}

impl FormRootVerticalAlignSchema {
    const TRAILER_SLOT: usize = 12;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer: &[&str],
    ) -> Option<Self> {
        let blocks = form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        Some(Self {
            trailer_slot: Self::TRAILER_SLOT.checked_add(blocks)?,
            children_align_trailer_slot: Self::CHILDREN_ALIGN_TRAILER_SLOT.checked_add(blocks)?,
        })
    }

    /// `ChildrenAlign` sits one member behind the vertical alignment in the
    /// same trailer, under the six-code table the grouping controls use.  Over
    /// all 5 139 form roots the export walks the member is `0` on the 5 134
    /// forms without the element and `1` on exactly the 5 that say `None`; the
    /// remaining five codes are the ones `UsualGroup` and `Page` already read
    /// off their own tuples, so the roots share one table rather than a second.
    const CHILDREN_ALIGN_TRAILER_SLOT: usize = 13;

    pub(crate) fn vertical_align(self, trailer: &[&str]) -> Option<FormRootVerticalAlign> {
        FormRootVerticalAlign::from_raw_value(trailer.get(self.trailer_slot)?.trim())
    }

    pub(crate) fn children_align(self, trailer: &[&str]) -> Option<&'static str> {
        match trailer.get(self.children_align_trailer_slot)?.trim() {
            "1" => Some("None"),
            "2" => Some("ItemsLeftTitlesLeft"),
            "3" => Some("ItemsRightTitlesLeft"),
            "4" => Some("ItemsLeftTitlesRight"),
            "5" => Some("ItemsRightTitlesRight"),
            "6" => Some("TitlesLeftDataAuto"),
            _ => None,
        }
    }

    pub(crate) const fn trailer_slot(self) -> usize {
        self.trailer_slot
    }

    pub(crate) fn accepts_raw_value(self, value: &str) -> bool {
        matches!(value.trim(), "0" | "1" | "2" | "3")
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormRootAutoUrlSchema {
    auto_url: Option<bool>,
}

impl FormRootAutoUrlSchema {
    /// The root trailer declares how many optional blocks sit ahead of
    /// `AutoURL`, and the flag is read from that declared count rather than
    /// from the trailer's overall length.
    ///
    /// Trailer member 2 is that count. Where it reads `0` the trailer is 24
    /// members long and `AutoURL` follows immediately at index 3; where it
    /// reads `1` a single `{22,...}` block sits between them, the trailer is
    /// 25 members long, and `AutoURL` moves to index 4. Both shapes are the
    /// same layout, so the slot is `3 + count`, never a per-arity constant:
    ///
    /// ```text
    /// count 0 (БСП):  "" | "" | 0 |              1 | "" | 0 0 0 0 0 0 | ...
    /// count 1 (УХ):   "" | "" | 1 | {22,{0},0,0,0,} | 0 | "" | 0 0 0 0 0 0 | ...
    ///                            ^         block      ^
    ///                          count                AutoURL
    /// ```
    ///
    /// Measured over all 18 634 root `50` forms the export walks on the stand,
    /// member 2 equals `trailer.len() - 24` on every single one, with no
    /// exception, and the member at `3 + count` is `0` on exactly the 587
    /// forms whose native document carries `<AutoURL>false</AutoURL>` and `1`
    /// on the other 18 047 -- no form reads anything else, and the two
    /// populations do not overlap:
    ///
    /// | corpus  | count | trailer | `0` (element present) | `1` (absent) |
    /// |---------|------:|--------:|----------------------:|-------------:|
    /// | wms     |     0 |      24 |                     0 |            5 |
    /// | sslbase |     0 |      24 |                    31 |          878 |
    /// | ssl     |     0 |      24 |                    44 |        1 119 |
    /// | ut      |     0 |      24 |                   231 |        4 970 |
    /// | mdm     |     1 |      25 |                     0 |            7 |
    /// | uh      |     1 |      25 |                   281 |       11 068 |
    ///
    /// Reading the count instead of trusting the length also keeps the search
    /// honest: `from_raw_layout` is handed whichever trailer the tail search
    /// validated, and a trailer whose declared count disagrees with its own
    /// length is refused rather than read at a guessed offset.
    ///
    /// Root `49` is deliberately left out. Its 1 543 ERP УХ roots put a braced
    /// group where root `50` keeps the count, and the 3 of them that carry the
    /// element are too thin a positive population to attribute a slot from, so
    /// they stay fail-closed.
    const AUTO_URL_SLOT_WITHOUT_OPTIONAL_BLOCKS: usize = 3;

    pub(crate) fn from_raw_layout(
        root_discriminator: Option<&str>,
        trailer: &[&str],
    ) -> Option<Self> {
        let blocks = form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        let slot = Self::AUTO_URL_SLOT_WITHOUT_OPTIONAL_BLOCKS.checked_add(blocks)?;
        let auto_url = match trailer.get(slot)?.trim() {
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
        let blocks = form_root_trailer_optional_blocks(root_discriminator, trailer)?;
        let group = match (
            header_group_marker.map(str::trim),
            trailer
                .get(Self::GROUP_KIND_SLOT.checked_add(blocks)?)
                .map(|field| field.trim()),
            trailer
                .get(Self::GROUP_VALUE_SLOT.checked_add(blocks)?)
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

    /// The 18-member track-bar option tuple, read off the whole population it
    /// has in UT 11.5.27.75 -- all 9 `TrackBarField` items of the
    /// configuration, joined to the platform's own element for each. Every
    /// member below is a total function of the platform answer on those 9:
    /// the value the platform prints where it prints one, and one fixed
    /// default where it prints nothing.
    ///
    /// * 1 `Width`, default `32` -- printed `35`, `9`, `20`, silent on the 6
    ///   that hold `32`. The arm used to admit `32` and so wrote a width on
    ///   all six.
    /// * 2 `Height`, default `2` -- printed `1` on 3, silent on the 6 that
    ///   hold `2`.
    /// * 5 `MinValue`, default `0`; 6 `MaxValue`, default `100`; 7 `Step`,
    ///   default `1`; 9 `LargeStep`, default `10`; 10 `MarkingStep`, default
    ///   `5`.
    /// * 11 `MarkingAppearance`: `1` on exactly the 3 items the platform
    ///   writes `TopLeft` on, `2` on the other 6. No other code occurs, so the
    ///   remaining appearances stay unread rather than guessed.
    /// * 13 `AutoMaxWidth`: `0` on the one item the platform writes `false`
    ///   on, `1` on the other 8.
    fn track_bar_dimension(options: &[&str], slot: usize, default: &str) -> Option<String> {
        let value = options.get(slot)?.trim();
        (value != default && value.parse::<i64>().is_ok()).then(|| value.to_string())
    }

    pub(crate) fn width(self, options: &[&str]) -> Option<String> {
        let value = options.get(1)?.trim();
        let is_non_default = match self.kind {
            FormSpecialFieldKind::ProgressBar | FormSpecialFieldKind::TrackBar => {
                value != "0" && value != "32"
            }
            FormSpecialFieldKind::Chart => false,
        };
        (is_non_default && value.parse::<u32>().is_ok()).then(|| value.to_string())
    }

    pub(crate) fn height(self, options: &[&str]) -> Option<String> {
        (self.kind == FormSpecialFieldKind::TrackBar)
            .then(|| Self::track_bar_dimension(options, 2, "2"))
            .flatten()
    }

    pub(crate) fn min_value(self, options: &[&str]) -> Option<String> {
        (self.kind == FormSpecialFieldKind::TrackBar)
            .then(|| Self::track_bar_dimension(options, 5, "0"))
            .flatten()
    }

    pub(crate) fn step(self, options: &[&str]) -> Option<String> {
        (self.kind == FormSpecialFieldKind::TrackBar)
            .then(|| Self::track_bar_dimension(options, 7, "1"))
            .flatten()
    }

    pub(crate) fn large_step(self, options: &[&str]) -> Option<String> {
        (self.kind == FormSpecialFieldKind::TrackBar)
            .then(|| Self::track_bar_dimension(options, 9, "10"))
            .flatten()
    }

    pub(crate) fn marking_step(self, options: &[&str]) -> Option<String> {
        (self.kind == FormSpecialFieldKind::TrackBar)
            .then(|| Self::track_bar_dimension(options, 10, "5"))
            .flatten()
    }

    pub(crate) fn marking_appearance(self, options: &[&str]) -> Option<&'static str> {
        matches!(
            (self.kind, options.get(11).map(|field| field.trim())),
            (FormSpecialFieldKind::TrackBar, Some("1"))
        )
        .then_some("TopLeft")
    }

    pub(crate) fn auto_max_width(self, options: &[&str]) -> Option<bool> {
        let slot = match self.kind {
            FormSpecialFieldKind::ProgressBar => 11,
            FormSpecialFieldKind::TrackBar => 13,
            FormSpecialFieldKind::Chart => return None,
        };
        (options.get(slot).map(|field| field.trim()) == Some("0")).then_some(false)
    }

    /// The progress bar keeps `HorizontalStretch` in the same option member the
    /// track bar does.  Over all 42 `ProgressBarField` records the export walks,
    /// member 3 is `1` on the 39 bars the platform writes no
    /// `<HorizontalStretch>` on and `0` on exactly the 3 that carry
    /// `<HorizontalStretch>false</HorizontalStretch>`; the member never carries
    /// any other value on this owner, so the raised state stays unread rather
    /// than guessed.
    pub(crate) fn horizontal_stretch(self, options: &[&str]) -> Option<bool> {
        match self.kind {
            FormSpecialFieldKind::TrackBar | FormSpecialFieldKind::ProgressBar
                if options.get(3).map(|field| field.trim()) == Some("0") =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    /// Slot 54 is the shared alignment slot of the 59-field field layout, so it
    /// goes through the shared table.  Pinning it to raw `1` had made the reader
    /// answer `Center` for an ordinal the corpus never carries here while
    /// answering nothing for the one progress bar that does carry the property -
    /// raw `2`, which the platform writes as `Bottom`.
    pub(crate) fn group_vertical_align(self, fields: &[&str]) -> Option<&'static str> {
        match self.kind {
            FormSpecialFieldKind::ProgressBar => form_group_vertical_align_xml(fields.get(54)?),
            _ => None,
        }
    }

    pub(crate) fn max_value(self, options: &[&str]) -> Option<String> {
        match self.kind {
            FormSpecialFieldKind::ProgressBar | FormSpecialFieldKind::TrackBar => {
                Self::track_bar_dimension(options, 6, "100")
            }
            FormSpecialFieldKind::Chart => None,
        }
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
    SpreadSheetDocumentField,
    HTMLDocumentField,
    CommandBar,
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
            "SpreadSheetDocumentField" => Self::SpreadSheetDocumentField,
            "HTMLDocumentField" => Self::HTMLDocumentField,
            "CommandBar" => Self::CommandBar,
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
    FieldPropertiesBeforeCommandSet,
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
                // `UsualGroup` obeys the same `field_count - 7` rule as its
                // sibling grouping controls; it used to be admitted through a
                // whitelist of the four shortest field counts (30, 32, 34, 36
                // -> 23, 25, 27, 29), which is that very rule spelled out four
                // times, so every longer group silently lost the property.  UT
                // 11.5.27.75, all 26 672 traced `UsualGroup` items: reverse
                // offset 7 is a total function of the platform answer -- `0` on
                // the 26 260 that carry nothing and `1`..`8` on the 412 that
                // carry `None`, `Balloon`, `Button`, `ShowAuto`, `ShowTop`,
                // `ShowLeft`, `ShowBottom` and `ShowRight`, with no code
                // mapping to two answers.
                | (FormTooltipRepresentationItemKind::UsualGroup, Some("5"))
                | (FormTooltipRepresentationItemKind::ButtonGroup, Some("6"))
                // A `CommandBar` is the wrapper-`22` item the same discriminator
                // slot codes `0`, and it obeys the very same `field_count - 7`
                // rule; it was simply never admitted.  UT 11.5.27.75, all 1 842
                // native command bars: reverse offset 7 reads `1` on exactly the
                // 3 the platform answers `<ToolTipRepresentation>None`, and `0`
                // on the other 1 839.  Reverse offset 8 reads `1` on 1 841 of
                // them, so the neighbouring slot is excluded outright rather
                // than merely unpreferred.
                | (FormTooltipRepresentationItemKind::CommandBar, Some("0"))
        );
        if admitted {
            return Some(FormTooltipRepresentationSchema {
                slot: field_count.checked_sub(7)?,
            });
        }
    }
    // A wrapper-`37` field carries its `ToolTipRepresentation` code at reverse
    // offset 9, not at the absolute slot 50 a 59-member record happens to spell
    // it in.  The record grows by one member when the item carries an extended
    // top-level head (`form_input_field_top_level_offset`), which also moves the
    // kind discriminator off slot 5 -- so the pairing this arm used to demand
    // could never match a 60-member record, and every such field silently lost
    // the property.  The kind alone decides admission here because
    // `form_child_item_tag` already derives the tag from that very
    // discriminator, read at its shifted position.
    //
    // Evidence, UT 11.5.27.75 native tree: at reverse offset 9 the 60-member
    // records answer non-`Omit` on exactly 16 items, and the platform prints
    // exactly those 16, value for value (`Button` 4, `ShowBottom` 8,
    // `ShowRight` 3, `Balloon` 1); the 59-member records keep the reading the
    // absolute slot 50 already gave them.  The same offset carries the two
    // document-field kinds the whitelist never named: it answers `None` on
    // exactly the 4 `HTMLDocumentField` and 2 `SpreadSheetDocumentField` items
    // the platform answers `None` on, and `Omit` on the other 172 and 220.
    if wrapper == "37"
        && matches!(
            item_kind,
            FormTooltipRepresentationItemKind::LabelField
                | FormTooltipRepresentationItemKind::InputField
                | FormTooltipRepresentationItemKind::CheckBoxField
                | FormTooltipRepresentationItemKind::PictureField
                | FormTooltipRepresentationItemKind::RadioButtonField
                | FormTooltipRepresentationItemKind::CalendarField
                | FormTooltipRepresentationItemKind::ProgressBarField
                | FormTooltipRepresentationItemKind::TrackBarField
                | FormTooltipRepresentationItemKind::ChartField
                | FormTooltipRepresentationItemKind::SpreadSheetDocumentField
                | FormTooltipRepresentationItemKind::HTMLDocumentField
        )
    {
        return Some(FormTooltipRepresentationSchema {
            slot: field_count.checked_sub(9)?,
        });
    }
    let slot = match (wrapper, field_count, item_kind, direct_discriminator) {
        ("31", 52, FormTooltipRepresentationItemKind::Button, _) => 30,
        // The prefixed `Button` record is the same record one slot later.
        ("31", 53, FormTooltipRepresentationItemKind::Button, _) => 31,
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
        // All 3 native command bars that carry the property write it directly
        // behind their title block and ahead of `HorizontalLocation` (2) and
        // `ExtendedTooltip` (3) -- the same site `Popup`/`Pages` use.
        FormTooltipRepresentationItemKind::CommandBar => {
            Some(FormTooltipRepresentationXmlOrder::AfterTitle)
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
        | FormTooltipRepresentationItemKind::ChartField
        // An `HTMLDocumentField` places it exactly where its sibling fields
        // do: behind `DataPath`/`SkipOnInput`/`TitleLocation` and ahead of the
        // geometry run (`Width`, `Height`, `MaxHeight`), `BorderColor`,
        // `ContextMenu` and `ExtendedTooltip`.
        | FormTooltipRepresentationItemKind::HTMLDocumentField => {
            Some(FormTooltipRepresentationXmlOrder::FieldProperties)
        }
        // A `SpreadSheetDocumentField` writes it one step earlier, ahead of its
        // own `CommandSet`: both native spreadsheet fields that carry the
        // property carry a command set too, and both write `DataPath`,
        // `TitleLocation`, `ToolTipRepresentation`, `CommandSet`,
        // `SelectionShowMode`, `ContextMenu`, `ExtendedTooltip` in that order.
        // (A `Table` writes the two the other way round, on all 18 that carry
        // both, but it has its own ordered property list.)
        FormTooltipRepresentationItemKind::SpreadSheetDocumentField => {
            Some(FormTooltipRepresentationXmlOrder::FieldPropertiesBeforeCommandSet)
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
    AllowInputEmptyMultipleValues,
    ListChoiceMode,
    ShowCheckBoxesInDropList,
    ExtendedEditMultipleValues,
    AutoMarkIncomplete,
}

pub(crate) const FORM_INPUT_FIELD_TAIL_XML_ORDER: &[FormInputFieldTailXmlProperty] = &[
    // `AllowInputEmptyMultipleValues` opens the tail: the one native item that
    // carries it puts it behind `DataPath`, `EditMode`, `Width` and
    // `HorizontalStretch` and ahead of `ExtendedEditMultipleValues`,
    // `ChoiceFoldersAndItems`, `ContextMenu` and `ExtendedTooltip`.
    FormInputFieldTailXmlProperty::AllowInputEmptyMultipleValues,
    FormInputFieldTailXmlProperty::ListChoiceMode,
    // `ShowCheckBoxesInDropList` trails `ListChoiceMode`, `ExtendedEdit`,
    // `ClearButton` (2), `ChoiceButton`, `MaxWidth` (2), `AutoMaxWidth`,
    // `Width`, `HorizontalStretch`, `TitleLocation` (2), `ToolTipRepresentation`
    // and `DataPath` (2), and leads `ChooseType`, `TextEdit`, `ChoiceList`,
    // `ContextMenu` (2), `ExtendedTooltip` (2) and `Events` (2), on the two
    // native items that carry it.  It never shares an item with
    // `AllowInputEmptyMultipleValues`, so their relative order is unobserved.
    FormInputFieldTailXmlProperty::ShowCheckBoxesInDropList,
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
    Enabled,
    Autofill,
    ReadOnly,
    SkipOnInput,
    DefaultItem,
    ChangeRowSet,
    ChangeRowOrder,
    Width,
    AutoMaxWidth,
    MaxWidth,
    Height,
    AutoMaxHeight,
    MaxHeight,
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
    HeightControlVariant,
    AutoMaxRowsCount,
    MaxRowsCount,
    TitleHeight,
    FooterHeight,
    Output,
    SearchOnInput,
    InitialListView,
    InitialTreeView,
    HorizontalStretch,
    VerticalStretch,
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
    TitleFont,
    Font,
    Shortcut,
    CommandSet,
    BehaviorOnHorizontalCompression,
    CurrentRowUse,
    ToolTip,
    ToolTipRepresentation,
    SearchStringLocation,
    ViewStatusLocation,
    SearchControlLocation,
    GroupHorizontalAlign,
    GroupVerticalAlign,
    RefreshRequest,
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
    // `TitleHeight` sits at the head of the table body: on all 5 native tables
    // that carry it, only `Representation` (2) and `TitleLocation` (1) precede
    // it, and it leads `ChangeRowSet` (2), `ChangeRowOrder` (1), `DefaultItem`
    // (1), `ReadOnly` (1), `HeaderHeight` (1), `RowSelectionMode` (1),
    // `UseAlternationRowColor` (5), `AutoInsertNewRow` (1), `EnableStartDrag`
    // (5), `DataPath` (5), `Title` (3) and `CommandSet` (2), with no pair
    // counted both ways.
    FormTableXmlProperty::TitleHeight,
    FormTableXmlProperty::CommandBarLocation,
    // `Enabled` opens the table's property run behind `Representation` (2) and
    // `CommandBarLocation` (2) and ahead of `ReadOnly` (1), `SelectionMode` (1),
    // `AutoInsertNewRow` (3), `FileDragMode` (3), `EnableDrag` (3),
    // `EnableStartDrag` (3), `RowFilter` (3), `DataPath` (3), `Title` (2) and
    // the three search/status locations (1 each), on all 3 native tables that
    // carry it, with no pair counted both ways.
    FormTableXmlProperty::Enabled,
    FormTableXmlProperty::Autofill,
    FormTableXmlProperty::ReadOnly,
    FormTableXmlProperty::SkipOnInput,
    FormTableXmlProperty::DefaultItem,
    FormTableXmlProperty::ChangeRowSet,
    FormTableXmlProperty::ChangeRowOrder,
    // The two caps sit inside the table's own geometry run, each directly
    // behind the auto flag it bounds, and not down in the shared visual tail
    // where every other table property had already been written.  UT
    // 11.5.27.75 native tree, 4 543 `Table` instances: `MaxWidth` (44) trails
    // `AutoMaxWidth` (36), `Width` (19), `ReadOnly` (19), `ChangeRowSet` (16),
    // `ChangeRowOrder` (16) and `CommandBarLocation` (14) and leads `Height`
    // (2), `AutoMaxHeight` (3), `MaxHeight` (3), `HeightInTableRows` (6),
    // `Header` (23), `AutoInsertNewRow` (27), `EnableStartDrag` (28),
    // `DataPath` (44), `Title` (18) and `CommandSet` (19); `MaxHeight` (34)
    // trails `AutoMaxHeight` (20), `Height` (7), `AutoMaxWidth` (6) and
    // `MaxWidth` (3) and leads `HeightInTableRows` (8), `Header` (21),
    // `AutoInsertNewRow` (21), `EnableStartDrag` (24), `DataPath` (34) and
    // `Title` (12).  No pair is observed in both directions.
    FormTableXmlProperty::Width,
    FormTableXmlProperty::AutoMaxWidth,
    FormTableXmlProperty::MaxWidth,
    FormTableXmlProperty::Height,
    FormTableXmlProperty::AutoMaxHeight,
    FormTableXmlProperty::MaxHeight,
    FormTableXmlProperty::HeightInTableRows,
    // The row-count trio closes the table's own height run.  UT 11.5.27.75
    // native tree: `HeightControlVariant` (43 owners) trails `HeightInTableRows`
    // (6), `AutoMaxHeight` (6), `MaxHeight` (4), `Height` (4), `MaxWidth` (12),
    // `AutoMaxWidth` (16), `Width` (9), `CommandBarLocation` (16), `ReadOnly`
    // (18), `ChangeRowSet` (18), `ChangeRowOrder` (19), `SkipOnInput` (5),
    // `TitleLocation` (4), `DefaultItem` (2), `UserVisible` (1), `Autofill` (1)
    // and `Representation` (40), and leads `AutoMaxRowsCount` (11),
    // `MaxRowsCount` (8), `Header` (20), `ChoiceMode` (1), `SelectionMode` (2),
    // `RowSelectionMode` (9), `HorizontalScrollBar` (5), `HorizontalLines` (10),
    // `VerticalLines` (10), `UseAlternationRowColor` (7), `Footer` (2),
    // `AutoInsertNewRow` (26) and `DataPath` (43).  `AutoMaxRowsCount` (35)
    // leads `MaxRowsCount` (17), and `MaxRowsCount` (27) leads `ChoiceMode` (1),
    // `SelectionMode` (6), `Header` (8) and `DataPath` (27).  No pair is
    // observed in both directions.
    FormTableXmlProperty::HeightControlVariant,
    FormTableXmlProperty::AutoMaxRowsCount,
    FormTableXmlProperty::MaxRowsCount,
    FormTableXmlProperty::ChoiceMode,
    FormTableXmlProperty::MultipleChoice,
    FormTableXmlProperty::RowInputMode,
    FormTableXmlProperty::SelectionMode,
    FormTableXmlProperty::RowSelectionMode,
    FormTableXmlProperty::Header,
    // `FooterHeight` follows the header switch and opens the line/scrollbar
    // run: on all 5 native tables that carry it, it trails `Representation`
    // (5), `ChangeRowSet` (3), `ChangeRowOrder` (3), `Width` (2),
    // `CommandBarLocation` (2), `TitleLocation` (1), `SkipOnInput` (1),
    // `MaxWidth` (1), `HeightInTableRows` (1), `Height` (1), `Header` (1) and
    // `AutoMaxWidth` (1), and leads `HorizontalScrollBar` (1),
    // `HorizontalLines` (1), `VerticalLines` (1), `AutoInsertNewRow` (3), the
    // stretch pair (1 each), `EnableStartDrag` (3), `EnableDrag` (3),
    // `FileDragMode` (3), `DataPath` (5), `RowPictureDataPath` (1),
    // `RowsPicture` (1), `Title` (4), `CommandSet` (3) and `RowFilter` (5).
    FormTableXmlProperty::FooterHeight,
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
    // `Output` closes the behaviour run: on all 6 native tables that carry it,
    // it trails `Representation` (6), `ReadOnly` (6), `InitialListView` (6),
    // `ChangeRowSet` (6), `ChangeRowOrder` (6), `SkipOnInput` (5),
    // `UseAlternationRowColor` (3), `SelectionMode` (3), `HorizontalLines` (2),
    // `HeightInTableRows` (2), `Header` (2), `CommandBarLocation` (2) and, once
    // each, `VerticalScrollBar`, `VerticalLines`, `TitleLocation`,
    // `SearchOnInput`, `MaxWidth`, `MaxRowsCount`, `InitialTreeView`,
    // `HorizontalScrollBar`, `AutoMaxRowsCount`, `AutoMarkIncomplete`,
    // `AutoInsertNewRow` and `AutoAddIncomplete`; it leads `VerticalStretch`
    // (1), `EnableStartDrag` (1), `EnableDrag` (1), `FileDragMode` (5),
    // `DataPath` (6), `RowPictureDataPath` (4), `RowsPicture` (4), `BackColor`
    // (1), `BorderColor` (5), `Title` (5), `CommandSet` (5),
    // `ToolTipRepresentation` (2), the three locations (2 each),
    // `CurrentRowUse` (1), `RowFilter` (5), `Events` (5) and `ChildItems` (6).
    FormTableXmlProperty::Output,
    // The stretch pair closes the layout run and opens the drag run.  Same
    // native tree: `HorizontalStretch` (35) trails `Header` (25),
    // `AutoInsertNewRow` (20), `HorizontalLines` (18), `VerticalLines` (16),
    // `AutoMaxHeight` (13), `MaxHeight` (10), `InitialTreeView` (10),
    // `MaxWidth` (5), `AutoAddIncomplete` (4), `UseAlternationRowColor` (3),
    // `InitialListView` (2) and `AutoMarkIncomplete` (2) and leads
    // `VerticalStretch` (17), `EnableStartDrag` (18), `EnableDrag` (18),
    // `FileDragMode` (18), `DataPath` (35), `RowPictureDataPath` (9), `Title`
    // (16) and `CommandSet` (13); `VerticalStretch` (53) trails
    // `AutoInsertNewRow` (32), `Header` (25), `HorizontalStretch` (17),
    // `AutoMaxHeight` (11), `MaxHeight` (8), `InitialTreeView` (3),
    // `AutoAddIncomplete` (2) and `InitialListView` (1) and leads
    // `EnableStartDrag` (39), `EnableDrag` (41), `FileDragMode` (26),
    // `DataPath` (53), `Title` (27) and `CommandSet` (20).  `SearchOnInput`
    // never shares a table with either, so it keeps its place ahead of them.
    FormTableXmlProperty::HorizontalStretch,
    FormTableXmlProperty::VerticalStretch,
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
    // A table's `TitleFont` follows its title block and precedes the command
    // set, like every other titled owner: it trails `Title` (22) and
    // `TitleTextColor` (11) and leads `CommandSet` (18), `RowFilter` (20),
    // `SearchStringLocation`, `ViewStatusLocation` and `SearchControlLocation`
    // (12 each) and `ToolTip` (1) on all 27 native occurrences.  `Font` (2)
    // trails `DataPath` and `FileDragMode` and leads `CommandSet`, `RowFilter`
    // and the three locations; it never shares a table with `Title`,
    // `TitleFont` or the colour triple, so it stays beside `TitleFont`, the
    // nearest position that satisfies every observed pair.
    FormTableXmlProperty::TitleFont,
    FormTableXmlProperty::Font,
    // A table's `Shortcut` sits between its title block and its command set.
    // UT 11.5.27.75 native tree, the 3 tables that carry one: it trails
    // `Title` (2), `RowPictureDataPath` (1), `DataPath` (3),
    // `EnableStartDrag` (3) and `Representation` (1), and leads `RowFilter`
    // (3), `CommandSet` (1), `ContextMenu` (3) and `AutoCommandBar` (3);
    // nothing this block writes ever follows it.
    FormTableXmlProperty::Shortcut,
    FormTableXmlProperty::CommandSet,
    // `BehaviorOnHorizontalCompression` follows `CommandSet` and leads
    // `RowFilter` on both evidenced occurrences: SSL/БСП 3.1.12.297's shared
    // `sslbase`/`ssl` `ДействияПриПолученииДанныхОбмена/.../НастройкаДействий`
    // and ERP УХ 3.2.12.6's `СообщенияФССОбИзмененииСостоянийЭЛН/.../ФормаСписка`,
    // both writing the same one spelling, `MoveItemsByImportance`. `RowFilter`
    // itself is emitted separately right after this ordered block (see the
    // `item.tag == "Table" && item.row_filter_nil` check in
    // `format_form_child_item_xml`), so this position alone reproduces the
    // evidenced order.
    FormTableXmlProperty::BehaviorOnHorizontalCompression,
    FormTableXmlProperty::ToolTip,
    FormTableXmlProperty::ToolTipRepresentation,
    FormTableXmlProperty::SearchStringLocation,
    FormTableXmlProperty::ViewStatusLocation,
    FormTableXmlProperty::SearchControlLocation,
    // A table's group alignment pair trails everything its property block
    // writes and leads `RowFilter`.  UT 11.5.27.75 native tree, the 4 tables
    // that carry one: `DataPath` leads it (4), and so do `Title` (3),
    // `CommandSet` (2), `ToolTipRepresentation` (1), `SearchStringLocation`,
    // `ViewStatusLocation` and `SearchControlLocation` (1 each); nothing this
    // block writes ever follows it.
    FormTableXmlProperty::GroupHorizontalAlign,
    FormTableXmlProperty::GroupVerticalAlign,
    FormTableXmlProperty::CurrentRowUse,
    // `RefreshRequest` closes the table's own scalar block, immediately ahead of
    // the `AutoRefresh` group.  UT 11.5.27.75 native tree, 4 543 `Table`
    // instances and 30 carrying the property: it trails `CommandBarLocation`
    // (28), `EnableStartDrag` (27), `ChangeRowOrder` (27), `SearchStringLocation`
    // (27), `Representation` (26), `InitialListView` (26), `CommandSet` (25),
    // `ViewStatusLocation` (25), `EnableDrag` (23), `Header` (22),
    // `SearchControlLocation` (22), `DataPath` (30) and `Title` (7), and leads
    // `AutoRefresh`, `AutoRefreshPeriod`, `Period`, `ChoiceFoldersAndItems`,
    // `RestoreCurrentRow`, `TopLevelParent`, `ShowRoot`, `AllowRootChoice`,
    // `UpdateOnDataChange` and `AllowGettingCurrentRowURL` (6 each), plus
    // `RowFilter` (23), `Events` (30) and `ChildItems` (30).  No pair is observed
    // in both directions, and `CurrentRowUse` never co-occurs with it.
    FormTableXmlProperty::RefreshRequest,
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
    Bottom,
    PullFromTop,
}

impl FormTableSearchStringLocation {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::CommandBar => "CommandBar",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::PullFromTop => "PullFromTop",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormTableViewStatusLocation {
    None,
    Top,
    Bottom,
}

impl FormTableViewStatusLocation {
    pub(crate) const fn xml_value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
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

/// Bounds of a `Table`'s counted property bag, read from the pair count alone.
///
/// The bag is a counted record list: slot 54 declares the pair count and the
/// pairs follow it.  The walk is the same whether or not the item also passes
/// the strict `FormTableSchema` shape test -- it reads cleanly on all 4 543
/// native `Table` items of UT 11.5.27.75 -- so the readers that only need one
/// bag key take this function rather than a schema they do not otherwise use.
pub(crate) fn form_table_counted_property_bag_bounds(fields: &[&str]) -> Option<(usize, usize)> {
    const PAIR_COUNT_SLOT: usize = 54;
    let pair_count = fields.get(PAIR_COUNT_SLOT)?.trim().parse::<usize>().ok()?;
    let start = PAIR_COUNT_SLOT.checked_add(1)?;
    let end = pair_count.checked_mul(2)?.checked_add(start)?;
    (end <= fields.len()).then_some((start, end))
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
    // Walking every `Table` item of the `sslbase`/`ssl` corpora (both SSL/БСП
    // 3.1.12.297) whose fixed tail is long enough to hold it: reverse offset 4
    // is `0` on all 642 that write no `<BehaviorOnHorizontalCompression>` and
    // `2` on the single one that does --
    // `ДействияПриПолученииДанныхОбмена/.../НастройкаДействий`'s
    // `ДействияПриПолучении` table, native `MoveItemsByImportance`. No other
    // code is observed at this coordinate. Independently, ERP УХ 3.2.12.6's
    // `SообщенияФССОбИзмененииСостоянийЭЛН/.../ФормаСписка` carries the
    // identical native spelling on its own table, corroborating the property
    // (not the exact raw coordinate, not re-derived from that corpus here).
    const BEHAVIOR_ON_HORIZONTAL_COMPRESSION_REVERSE_OFFSET: usize = 4;
    // This scalar is part of the fixed tail, not the counted property bag.
    // Native ibcmd emits AutoMaxWidth=false only for raw code 0 here.
    const AUTO_MAX_WIDTH_REVERSE_OFFSET: usize = 15;
    // Fixed tail scalar: 0=false, 1=true, 2=platform default (omitted).
    const AUTO_ADD_INCOMPLETE_REVERSE_OFFSET: usize = 36;
    /// The height/row-count run at the very end of the fixed tail, directly
    /// behind `CurrentRowUse` (reverse offset 5).  Same corpus, same totality:
    /// reverse offset 8 is `0` on the 4 485 tables without a
    /// `<HeightControlVariant>` and `1`/`2`/`3` on the 7/10/26 that say
    /// `UseHeightInFormRows`, `UseHeightInTableRows` and `UseContentHeight`;
    /// reverse offset 7 is `1` on the 4 493 without an `<AutoMaxRowsCount>` and
    /// `0` on all 35 that say `false`; reverse offset 6 is `0` on the 4 501
    /// without a `<MaxRowsCount>` and the written count itself on all 27 that
    /// carry one.  No code maps to two different answers in any of the three.
    const HEIGHT_CONTROL_VARIANT_REVERSE_OFFSET: usize = 8;
    const AUTO_MAX_ROWS_COUNT_REVERSE_OFFSET: usize = 7;
    const MAX_ROWS_COUNT_REVERSE_OFFSET: usize = 6;
    const TITLE_HEIGHT_SLOT: usize = 7;
    const FOOTER_HEIGHT_SLOT: usize = 29;
    const OUTPUT_SLOT: usize = 40;

    /// `AutoMarkIncomplete` is the fixed-tail scalar directly ahead of
    /// `AutoAddIncomplete`, sharing its tri-state code map.
    ///
    /// No forward slot explains it, because the columns and the counted property
    /// bag between them make the table layout variable-length. Read from the end
    /// over all 4 528 `Table` items of the native UT 11.5.27.75 form bodies,
    /// reverse offset 37 is a total function: `2` on the 4 481 tables the
    /// platform writes nothing on, `1` on exactly the 36 it writes `true` on and
    /// `0` on exactly the 11 it writes `false` on, with no counter-example and
    /// no fourth code.
    const AUTO_MARK_INCOMPLETE_REVERSE_OFFSET: usize = 37;
    /// `Enabled` sits in the same plain top-level slot the field kinds use.
    ///
    /// Over the same 4 528 `Table` items slot 13 reads `1` on the 4 525 tables
    /// with no `<Enabled>` and `0` on exactly the 3 that carry
    /// `<Enabled>false</Enabled>`, with no other code.
    const ENABLED_SLOT: usize = 13;
    /// `RefreshRequest` is a fixed-tail scalar addressed from the end, which is
    /// why no forward slot explains it.  UT 11.5.27.75 native tree, 4 509 traced
    /// tables: reverse offset 16 is a total function - `1` on all 30 tables whose
    /// native document says `PullFromTop` and `0` on the other 4 479, with no
    /// counter-example.
    const REFRESH_REQUEST_REVERSE_OFFSET: usize = 16;

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
        form_table_counted_property_bag_bounds(fields)
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
            // Read off the platform, not interpolated: of the 4 543 traced
            // `Table` items of UT 11.5.27.75 exactly one holds `5` here, and
            // the platform writes `<TitleLocation>Bottom</TitleLocation>` on
            // exactly that table. The remaining ordinals stay unread.
            "5" => Some("Bottom"),
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
        // The slot is a total function of the native spelling over all 4 543
        // `Table` items of UT 11.5.27.75: `0` on the 3 934 that write nothing,
        // `1` on all 505 that say `None`, `3` on all 56 that say `Top`, `2` on
        // all 44 that say `CommandBar`, `6` on all 3 that say `PullFromTop`
        // and `4` on the one that says `Bottom`.  The last two codes had no
        // spelling, so those four tables lost the element.
        match fields.get(slot)?.trim() {
            "1" => Some(FormTableSearchStringLocation::None),
            "2" => Some(FormTableSearchStringLocation::CommandBar),
            "3" => Some(FormTableSearchStringLocation::Top),
            "4" => Some(FormTableSearchStringLocation::Bottom),
            "6" => Some(FormTableSearchStringLocation::PullFromTop),
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
        // The code is a total function of the native spelling over both
        // configurations whose form layouts were censused, and neither
        // contradicts the other.  Документооборот КОРП 3.0.21.3, 1 551 tables
        // paired with their layout record by the item's own name: `0` on the
        // 903 that write nothing, `1` on all 619 that say `None`, `2` on all 18
        // that say `Top` and `3` on all 11 that say `Bottom`.  ERP УХ 3.2.12.6,
        // 7 967 tables: `0` on 5 362, `1` on 2 529 and `2` on 76, with the same
        // spellings and no `3` at all.  `3` had no spelling, so those 11 tables
        // lost the element.
        match fields.get(slot)?.trim() {
            "1" => Some(FormTableViewStatusLocation::None),
            "2" => Some(FormTableViewStatusLocation::Top),
            "3" => Some(FormTableViewStatusLocation::Bottom),
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

    pub(crate) fn behavior_on_horizontal_compression_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::BEHAVIOR_ON_HORIZONTAL_COMPRESSION_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "2").then_some(slot)
    }

    /// `<BehaviorOnHorizontalCompression>`, evidenced only as
    /// `MoveItemsByImportance` (raw `2`); raw `0` is the platform default and
    /// writes nothing. See `BEHAVIOR_ON_HORIZONTAL_COMPRESSION_REVERSE_OFFSET`
    /// for the evidence.
    pub(crate) fn behavior_on_horizontal_compression(
        self,
        fields: &[&str],
    ) -> Option<&'static str> {
        (fields
            .get(self.behavior_on_horizontal_compression_slot(fields)?)?
            .trim()
            == "2")
            .then_some("MoveItemsByImportance")
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

    pub(crate) fn auto_mark_incomplete_slot(self, fields: &[&str]) -> Option<usize> {
        let slot = fields
            .len()
            .checked_sub(Self::AUTO_MARK_INCOMPLETE_REVERSE_OFFSET)?;
        matches!(fields.get(slot)?.trim(), "0" | "1" | "2").then_some(slot)
    }

    pub(crate) fn auto_mark_incomplete(self, fields: &[&str]) -> Option<bool> {
        match fields.get(self.auto_mark_incomplete_slot(fields)?)?.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }

    pub(crate) fn enabled(self, fields: &[&str]) -> Option<bool> {
        (fields.get(Self::ENABLED_SLOT)?.trim() == "0").then_some(false)
    }

    pub(crate) fn height_control_variant(self, fields: &[&str]) -> Option<&'static str> {
        let slot = fields
            .len()
            .checked_sub(Self::HEIGHT_CONTROL_VARIANT_REVERSE_OFFSET)?;
        match fields.get(slot)?.trim() {
            "1" => Some("UseHeightInFormRows"),
            "2" => Some("UseHeightInTableRows"),
            "3" => Some("UseContentHeight"),
            _ => None,
        }
    }

    pub(crate) fn auto_max_rows_count(self, fields: &[&str]) -> Option<bool> {
        let slot = fields
            .len()
            .checked_sub(Self::AUTO_MAX_ROWS_COUNT_REVERSE_OFFSET)?;
        (fields.get(slot)?.trim() == "0").then_some(false)
    }

    pub(crate) fn max_rows_count(self, fields: &[&str]) -> Option<String> {
        let slot = fields
            .len()
            .checked_sub(Self::MAX_ROWS_COUNT_REVERSE_OFFSET)?;
        let value = fields.get(slot)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_owned())
    }

    /// The three title/footer/output scalars of the table header, each a total
    /// function of the platform answer over all 4 542 traced `Table` items:
    /// slot 7 is `0` on the 4 537 tables without a `<TitleHeight>` and the
    /// written height on all 5 that carry one; slot 29 is `1` on the 4 537
    /// without a `<FooterHeight>` and the written height - `0`, `2` or `3` - on
    /// all 5 that carry one; slot 40 is `0` on the 4 536 without an
    /// `<Output>`, `1` on the 5 that say `Enable` and `2` on the one that says
    /// `Disable`.
    pub(crate) fn title_height(self, fields: &[&str]) -> Option<String> {
        let value = fields.get(Self::TITLE_HEIGHT_SLOT)?.trim();
        (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_owned())
    }

    pub(crate) fn footer_height(self, fields: &[&str]) -> Option<String> {
        let value = fields.get(Self::FOOTER_HEIGHT_SLOT)?.trim();
        (value != "1" && value.parse::<u32>().is_ok()).then(|| value.to_owned())
    }

    pub(crate) fn output(self, fields: &[&str]) -> Option<&'static str> {
        form_output_code(fields.get(Self::OUTPUT_SLOT).copied())
    }

    pub(crate) fn refresh_request(self, fields: &[&str]) -> Option<&'static str> {
        let slot = fields
            .len()
            .checked_sub(Self::REFRESH_REQUEST_REVERSE_OFFSET)?;
        // Same pairing, same two configurations.  Документооборот КОРП
        // 3.0.21.3, 1 551 tables: `0` on the 1 503 that write nothing, `1` on
        // all 43 that say `PullFromTop` and `3` on all 5 that say
        // `PullFromTopOrBottom`.  ERP УХ 3.2.12.6, 7 967 tables: `0` on 7 627
        // and `1` on all 30 that say `PullFromTop`, with no `3`.  Code `2` is
        // observed nowhere and stays unspelled rather than being guessed into
        // the run.
        match fields.get(slot)?.trim() {
            "1" => Some("PullFromTop"),
            "3" => Some("PullFromTopOrBottom"),
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
    /// The conditional `UserVisible`-common prefix shifts every top-level
    /// member of the record by one, and this reader addresses three of them --
    /// the record length, the discriminator and the `DefaultItem` flag.  Its
    /// guard used to spell all three at offset `0`, so a shifted record was
    /// refused outright and the field lost `VerticalScrollBar`,
    /// `HorizontalScrollBar`, `ViewScalingMode`, `Output`, `Protection`, the
    /// geometry pair and everything else this tuple carries -- 27 ERP УХ forms
    /// whose whole remaining diff is exactly that.  The offset is the one
    /// `FormFieldSchema` already computed and stored to reach this tuple at
    /// all (`OPTIONS_BASE_SLOT + offset`); the *option* slots do not shift,
    /// because the shift is on the record, not inside the tuple.
    fn from_raw_layout(fields: &[&str], options: &[&str], top_level_offset: usize) -> Option<Self> {
        if fields.len() != 59 + top_level_offset
            || fields.get(5 + top_level_offset).map(|field| field.trim()) != Some("6")
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
            default_item: (fields.get(16 + top_level_offset)?.trim() == "1").then_some(true),
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
            // Slot 12 is one code, not a flag with a hole in it, and it uses
            // the same `1`/`2` pairing the `Table` header does: of the 184
            // `SpreadSheetDocumentField` option tuples UT 11.5.27.75 spells
            // out, 180 hold `0` and write nothing, 3 hold `1` and write
            // `Enable`, and the one that holds `2` is the one the platform
            // writes `<Output>Disable</Output>` on.  Reading only `1` lost it.
            output: form_output_code(option(12)),
            protection: explicit_true(10),
            enable_start_drag: explicit_false(16),
            enable_drag: explicit_false(17),
            // Of the 222 `SpreadSheetDocumentField` option tuples UT
            // 11.5.27.75 spells out, slot 19 holds `1` on exactly the 40 items
            // the platform writes `<ViewScalingMode>Normal</ViewScalingMode>`
            // on and `0` on the other 182, with no miss on either side.  The
            // slot had no reader, so none of the 40 was ever written.
            view_scaling_mode: (option(19) == Some("1")).then_some("Normal"),
            // Slot 14 is the group ruler switch: 218 of the 222 native
            // `SpreadSheetDocumentField` option tuples hold `1` and carry no
            // `<ShowGroups>`, and the 4 that hold `0` are exactly the 4 the
            // platform writes `<ShowGroups>false</ShowGroups>` on.  No other
            // code occurs.
            show_groups: explicit_false(14),
            // The last slot of the tuple: `2` on the 221 items with no
            // `<DrawingSelectionShowMode>` and `0` on the one that says
            // `Show`.  No other code occurs, so the two remaining codes are
            // unobserved and go unread rather than guessed.
            drawing_selection_show_mode: match option(31) {
                Some("0") => Some("Show"),
                _ => None,
            },
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
            // `RowFilter` is key 13, not key 10.  Walking the counted bag of
            // all 4 543 native `Table` items of UT 11.5.27.75 (the walk reads
            // cleanly on every one of them): key 13 is present, and always as
            // the undefined marker `{"U"}`, on exactly the 1 986 tables whose
            // document carries `<RowFilter xsi:nil="true"/>` and absent on the
            // other 2 557.  Key 10 also holds `{"U"}` -- on 1 947 tables, none
            // of which writes a `<RowFilter>` -- so reading it as the filter
            // answered for a different member on every dynamic list.
            Self::RowFilter => "13",
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
    ChoiceForm,
    ChoiceParameterLinks,
    ChoiceParameters,
    AutoChoiceIncomplete,
    AutoMarkIncomplete,
    ChooseType,
    IncompleteChoiceMode,
    AvailableTypes,
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
    AutoShowClearButtonMode,
    AutoCorrectionOnTextInput,
    SpellCheckingOnTextInput,
    SpecialTextInputMode,
    MultipleValuesOptions,
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
            // The slot immediately ahead of the mirrored link collections holds
            // the chosen selection form as a design-time object identifier. Over
            // all 49 916 `InputField` option tuples of the native UT 11.5.27.75
            // form bodies it takes 20 values and is a total function of the
            // platform's `<ChoiceForm>`: the nil identifier on 49 839 tuples,
            // none of whose items carries the element, and 19 non-nil
            // identifiers that map one-to-one onto the 19 distinct references
            // the platform writes, with no item missing on either side and no
            // identifier ever spelled two ways.
            Self::ChoiceForm => 25,
            Self::ChoiceParameterLinks => 26,
            Self::ChoiceParameters => 27,
            Self::AutoCellHeight => 28,
            Self::AutoChoiceIncomplete => 28,
            Self::Format => 29,
            Self::EditFormat => 30,
            Self::AutoMarkIncomplete => 31,
            Self::ChooseType => 32,
            Self::IncompleteChoiceMode => 33,
            // The slot holds an ordinary serialized type pattern, and it is the
            // only slot of the tuple that does: sweeping every slot of every one
            // of the 49 951 `InputField` option tuples of the native UT
            // 11.5.27.75 forms, slot 34 parses as a type pattern on all 49 951
            // and no other slot parses on any. It is empty (`{"Pattern"}`) on
            // 49 937 of them and carries types on 14 - exactly, item for item,
            // the 14 `InputField` items across 13 forms on which the platform
            // writes an `<AvailableTypes>` block, with no item missing from
            // either side. The decoded type sequence equals the platform's own
            // `<v8:Type>` sequence on all 14, including the one item that lists
            // five types with three qualifier groups.
            Self::AvailableTypes => 34,
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
            // `2 -> FilledOnly`, `0` writes nothing.  Of the 50 065
            // `InputField` option tuples the UT 11.5.27.75 form bodies spell
            // out, 50 060 hold `0` here and none of their items carries an
            // `<AutoShowClearButtonMode>`; the 5 that hold `2` are, item for
            // item, exactly the 5 the platform writes `FilledOnly` on.  No
            // other code occurs in the slot.
            Self::AutoShowClearButtonMode => 55,
            // `2 -> DontUse`, `0` writes nothing: 50 064 tuples hold `0` and
            // write nothing, the one that holds `2` is the one item the
            // platform writes `<AutoCorrectionOnTextInput>DontUse</...>` on,
            // and no other code occurs.
            Self::AutoCorrectionOnTextInput => 57,
            // The same two codes one slot further on, and the same score:
            // 50 064 zeros with no element, one `2` on the one item that says
            // `<SpellCheckingOnTextInput>DontUse</...>`.
            Self::SpellCheckingOnTextInput => 58,
            // `4 -> Email`, `5 -> PhoneNumber`, `6 -> Digits`, `0` writes
            // nothing.  50 058 tuples hold `0` and carry no
            // `<SpecialTextInputMode>`; the 5 fours, the one five and the one
            // six are, item for item, exactly the 7 items the platform writes
            // the three spellings on, with no other code in the slot.
            Self::SpecialTextInputMode => 60,
            // The multiple-values sub-tuple.  It is the bare `{0}` on 50 058
            // of the 50 065 tuples, none of whose items carries either of the
            // two properties below, and a seven-member record on the other 7.
            Self::MultipleValuesOptions => 62,
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
    PasswordMode,
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
            // Of the 25 156 traced `LabelField` option tuples of UT
            // 11.5.27.75, slot 11 reads `2` on the 25 154 that carry no
            // `<PasswordMode>` and `0` on the one the platform writes
            // `<PasswordMode>false</PasswordMode>` on.  No other code occurs,
            // so the raised state stays unread rather than guessed.
            Self::PasswordMode => 11,
            Self::AutoMaxWidth => 15,
            Self::MaxWidth => 16,
            Self::AutoMaxHeight => 18,
            Self::MaxHeight => 19,
        }
    }
}

#[cfg(test)]
mod unemitted_property_tests {
    use super::*;

    /// A `Page` (discriminator `4`) carrying the conditional `UserVisible`
    /// prefix is a valid grouping layout; the discriminator whitelist used to
    /// drop it, and with it the whole page and its subtree.
    ///
    /// `CommandBar` (`0`), `Popup` (`1`) and `ButtonGroup` (`6`) were folded
    /// into the admitted set alongside `ColumnGroup`/`Pages`/`Page`/
    /// `UsualGroup` once real ERP УХ 3.2.12.6 bytes proved they carry the
    /// identical prefix tuple on the identical `31 + 2k` progression --
    /// see `docs/evidence` and this crate's
    /// `form-conditional-group-command-bar-popup-buttongroup` fixture.
    /// `ContextMenu`/`AutoCommandBar` (`8`/`9`) genuinely stay out *of this
    /// arm*: they carry the same tuple on a different, one-shorter `30 + 2k`
    /// floor (the arm below), so at these odd `31 + 2k` counts neither arm
    /// admits them.
    #[test]
    fn conditional_group_prefix_admits_pages() {
        for count in [31usize, 33, 35, 37] {
            for discriminator in ["0", "1", "2", "3", "4", "5", "6"] {
                assert!(
                    FormConditionalGroupSchema::from_raw_layout(
                        "22",
                        count,
                        Some(false),
                        Some(discriminator),
                    )
                    .is_some(),
                    "count {count} discriminator {discriminator}"
                );
            }
            // `ContextMenu`/`AutoCommandBar` carry the prefix on the
            // shorter `30 + 2k` floor (the arm below), never this one.
            for discriminator in ["8", "9"] {
                assert!(
                    FormConditionalGroupSchema::from_raw_layout(
                        "22",
                        count,
                        Some(false),
                        Some(discriminator),
                    )
                    .is_none(),
                    "count {count} discriminator {discriminator}"
                );
            }
        }
        // The length progression is unchanged: an even offset past 31 is required.
        assert!(
            FormConditionalGroupSchema::from_raw_layout("22", 34, Some(false), Some("4")).is_none()
        );
        // A page without the prefix marker is not a conditional layout.
        assert!(FormConditionalGroupSchema::from_raw_layout("22", 33, None, Some("4")).is_none());
    }

    /// A `UsualGroup` reads its tooltip representation at `field_count - 7`,
    /// like every other grouping control.  The old whitelist stopped at 36.
    #[test]
    fn usual_group_tooltip_representation_follows_the_length_progression() {
        for count in [30usize, 32, 34, 36, 38, 40, 52] {
            let schema = form_tooltip_representation_schema("22", count, "UsualGroup", Some("5"))
                .unwrap_or_else(|| panic!("field count {count}"));
            assert_eq!(schema.slot(), count - 7, "field count {count}");
        }
        // An odd count is not a member of the progression.
        assert!(form_tooltip_representation_schema("22", 37, "UsualGroup", Some("5")).is_none());
        // A discriminator that does not belong to `UsualGroup` still fails.
        assert!(form_tooltip_representation_schema("22", 38, "UsualGroup", Some("2")).is_none());
    }

    /// `Shape` (reverse offset 8) and `PictureLocation` (reverse offset 5) of a
    /// `Button`, at both observed field counts.
    #[test]
    fn button_shape_and_picture_location_read_the_fixed_tail() {
        for count in [52usize, 53] {
            let schema = FormButtonShapeSchema::from_raw_layout("31", count, "Button")
                .unwrap_or_else(|| panic!("field count {count}"));
            let mut fields = vec!["0"; count];
            assert_eq!(schema.shape(&fields), None);
            assert_eq!(schema.picture_location(&fields), None);
            fields[count - 8] = "1";
            fields[count - 5] = "1";
            assert_eq!(schema.shape(&fields), Some("Usual"));
            assert_eq!(schema.picture_location(&fields), Some("Left"));
            fields[count - 8] = "2";
            fields[count - 5] = "2";
            assert_eq!(schema.shape(&fields), Some("Oval"));
            assert_eq!(schema.picture_location(&fields), Some("Right"));
        }
        assert!(FormButtonShapeSchema::from_raw_layout("31", 51, "Button").is_none());
        assert!(FormButtonShapeSchema::from_raw_layout("22", 52, "Popup").is_none());
    }

    /// A `Popup` keeps its shape representation in member 6 of its nine-member
    /// option tuple, with the same code table `Button` uses.
    #[test]
    fn popup_shape_representation_reads_the_option_tuple() {
        let mut options = vec![
            "7",
            r#"{4,0,{0},"",-1,-1,1,0,""}"#,
            "{0}",
            "2",
            "3",
            "0",
            "0",
            "{3,4,{0}}",
            "{3,4,{0}}",
        ];
        let schema = FormPopupShapeRepresentationSchema::from_raw_layout("22", "Popup", &options)
            .expect("observed popup option tuple");
        assert_eq!(schema.shape_representation(&options), None);
        for (code, expected) in [("1", "Always"), ("2", "WhenActive"), ("3", "None")] {
            options[6] = code;
            assert_eq!(schema.shape_representation(&options), Some(expected));
        }
        // A tuple of another shape is not a popup option tuple.
        assert!(
            FormPopupShapeRepresentationSchema::from_raw_layout("22", "Popup", &options[..8])
                .is_none()
        );
        assert!(
            FormPopupShapeRepresentationSchema::from_raw_layout("22", "Pages", &options).is_none()
        );
    }

    /// The six `SearchStringAddition` properties, read off the observed
    /// eleven-member option tuple and the two top-level slots.
    #[test]
    fn search_string_addition_reads_its_geometry_and_alignment() {
        let mut options = vec![
            "1",
            "0",
            "2",
            "{3,4,{0}}",
            "{3,4,{0}}",
            "{3,4,{0}}",
            "{7,3,0,1,100}",
            "{0,1,0}",
            "1",
            "0",
            "0",
        ];
        let mut fields = vec!["0"; 24];
        fields[11] = "0";
        fields[21] = "3";
        let schema = FormSearchStringAdditionSchema::from_raw_layout(
            "5",
            24,
            "SearchStringAddition",
            &options,
        )
        .expect("observed addition layout");
        let quiet = schema.properties(&fields, &options);
        assert_eq!(quiet.width, None);
        assert_eq!(quiet.max_width, None);
        assert_eq!(quiet.horizontal_stretch, None);
        assert_eq!(quiet.auto_max_width, None);
        assert_eq!(quiet.group_horizontal_align, None);
        assert_eq!(quiet.tooltip_representation, None);

        options[1] = "40";
        options[2] = "0";
        options[8] = "0";
        options[9] = "26";
        fields[11] = "3";
        fields[21] = "2";
        let loud = schema.properties(&fields, &options);
        assert_eq!(loud.width.as_deref(), Some("40"));
        assert_eq!(loud.max_width.as_deref(), Some("26"));
        assert_eq!(loud.horizontal_stretch, Some(false));
        assert_eq!(loud.auto_max_width, Some(false));
        assert_eq!(loud.group_horizontal_align, Some("Right"));
        assert_eq!(loud.tooltip_representation, Some("Button"));

        assert!(
            FormSearchStringAdditionSchema::from_raw_layout(
                "5",
                24,
                "ViewStatusAddition",
                &options
            )
            .is_none()
        );
        assert!(
            FormSearchStringAdditionSchema::from_raw_layout(
                "5",
                26,
                "SearchStringAddition",
                &options
            )
            .is_none()
        );
    }
}

#[cfg(test)]
mod table_tail_property_tests {
    use super::*;

    /// A minimal `Table` record of the observed shape: wrapper `55`, first
    /// field `55`, 99 base fields plus an even-sized suffix, every slot the
    /// layout gate reads holding a code it accepts.
    fn table_fields(len: usize) -> Vec<&'static str> {
        assert!(len >= 99 && (len - 99) % 2 == 0);
        let mut fields = vec!["0"; len];
        fields[0] = "55";
        // FormTableSlot codes the gate insists on.
        for slot in [
            12usize, 14, 16, 17, 18, 22, 23, 24, 25, 26, 28, 30, 32, 33, 36, 37, 38, 39, 52, 53,
        ] {
            fields[slot] = "0";
        }
        fields[19] = "10"; // Width
        fields[20] = "10"; // Height
        fields[54] = "0"; // empty counted property bag
        // Fixed-tail scalars the gate parses.
        fields[len - 2] = "0"; // FileDragMode
        fields[len - 34] = "0"; // MultipleChoice
        fields[len - 30] = "0"; // SkipOnInput
        fields[len - 29] = "0"; // SearchOnInput
        fields[len - 5] = "0"; // CurrentRowUse
        fields[len - 15] = "1"; // AutoMaxWidth
        fields[len - 36] = "2"; // AutoAddIncomplete
        // The four properties under test, in their "writes nothing" codes.
        fields[len - 37] = "2"; // AutoMarkIncomplete
        fields[len - 8] = "0"; // HeightControlVariant
        fields[len - 7] = "1"; // AutoMaxRowsCount
        fields[len - 6] = "0"; // MaxRowsCount
        fields[7] = "0"; // TitleHeight
        fields[29] = "1"; // FooterHeight
        fields[40] = "0"; // Output
        fields
    }

    #[test]
    fn table_tail_scalars_are_read_at_their_reverse_offsets() {
        for len in [99usize, 101, 141] {
            let mut fields = table_fields(len);
            let schema = FormTableSchema::from_raw_layout("55", "Table", &fields)
                .unwrap_or_else(|| panic!("field count {len}"));
            assert_eq!(schema.auto_mark_incomplete(&fields), None);
            assert_eq!(schema.height_control_variant(&fields), None);
            assert_eq!(schema.auto_max_rows_count(&fields), None);
            assert_eq!(schema.max_rows_count(&fields), None);
            assert_eq!(schema.title_height(&fields), None);
            assert_eq!(schema.footer_height(&fields), None);
            assert_eq!(schema.output(&fields), None);

            fields[len - 37] = "1";
            fields[len - 8] = "3";
            fields[len - 7] = "0";
            fields[len - 6] = "13";
            fields[7] = "5";
            fields[29] = "0";
            fields[40] = "2";
            let schema = FormTableSchema::from_raw_layout("55", "Table", &fields)
                .unwrap_or_else(|| panic!("field count {len}"));
            assert_eq!(schema.auto_mark_incomplete(&fields), Some(true));
            assert_eq!(
                schema.height_control_variant(&fields),
                Some("UseContentHeight")
            );
            assert_eq!(schema.auto_max_rows_count(&fields), Some(false));
            assert_eq!(schema.max_rows_count(&fields).as_deref(), Some("13"));
            assert_eq!(schema.title_height(&fields).as_deref(), Some("5"));
            assert_eq!(schema.footer_height(&fields).as_deref(), Some("0"));
            assert_eq!(schema.output(&fields), Some("Disable"));

            fields[len - 37] = "0";
            fields[len - 8] = "1";
            let schema = FormTableSchema::from_raw_layout("55", "Table", &fields).unwrap();
            assert_eq!(schema.auto_mark_incomplete(&fields), Some(false));
            assert_eq!(
                schema.height_control_variant(&fields),
                Some("UseHeightInFormRows")
            );
            fields[len - 8] = "2";
            let schema = FormTableSchema::from_raw_layout("55", "Table", &fields).unwrap();
            assert_eq!(
                schema.height_control_variant(&fields),
                Some("UseHeightInTableRows")
            );
        }
    }

    /// A code outside the observed table fails closed: the property is dropped,
    /// never guessed.
    #[test]
    fn unknown_tail_codes_write_nothing() {
        let len = 99usize;
        let mut fields = table_fields(len);
        fields[len - 8] = "9";
        fields[len - 37] = "7";
        fields[40] = "9";
        fields[len - 6] = "x";
        let schema = FormTableSchema::from_raw_layout("55", "Table", &fields).unwrap();
        assert_eq!(schema.height_control_variant(&fields), None);
        assert_eq!(schema.auto_mark_incomplete(&fields), None);
        assert_eq!(schema.output(&fields), None);
        assert_eq!(schema.max_rows_count(&fields), None);
    }
}
