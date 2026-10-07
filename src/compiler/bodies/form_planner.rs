//! The embedded planner a `pl:Planner`-typed form attribute stores.
//!
//! A form attribute whose `<Settings xsi:type="pl:Planner">` carries a design
//! stores it in member 14 of its record (`NativeFormAttribute.trailing[0]`),
//! the slot every attribute keeps its settings in:
//!
//! ```text
//! {0,1,"Planner",{"#",43dc7f37-…,{8,<item count>,<item>*,<31 tail members>}}}
//! ```
//!
//! This module is the exact inverse of the exporter's decoder
//! (`parse_form_planner_settings_xml` and `format_form_planner_settings_xml`
//! in `mssql_dump::form_body`): every member that decoder publishes is read
//! from the element it publishes, every member it validates against a literal
//! is written as that literal, and the XML is consumed in the order the
//! decoder writes it, so an element the decoder would not have written -- or
//! would have written elsewhere -- is refused rather than guessed.
//!
//! The evidence is the decoder's: the thirteen records of
//! `tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings` -- ten
//! `plx-*` seeds, each the control tree with one axis changed and loaded by
//! platform 8.3.27.2214 itself from XML, and the three native planner
//! attributes of Документооборот КОРП 3.0.21.3 -- and this writer rebuilds
//! every one of them byte for byte from its native `<Settings>` element
//! (`rebuilds_every_platform_proven_planner_record`). The seeds are what the
//! platform stores when it loads the XML, which is exactly what a load has to
//! write; the three native records agree with them member for member.
//!
//! Two spellings the decoder accepts and the seeds do not store are not
//! written: a border whose style id is the nil uuid (Документооборот's
//! planner forms carry one, the platform writes `48312c09-…` on load) and a
//! `<pl:itemsBehaviorWhenSpaceInsufficient>` other than `CollapseItems`,
//! the one ordinal the XDTO enumeration names.
//!
//! Every value this writer returns is checked by running it back through the
//! exporter's own decoder: a planner that does not export back to the XML it
//! was built from is refused.

use anyhow::{Result, anyhow, bail, ensure};

use super::form_chart::{
    BORDER_UUID, LINE_UUID, XmlNode, attributes_are, boolean, code, color, font, integer, leaf,
    localized, parse_xml, string, style_of, verify_round_trip, width_of,
};

/// The namespace the `pl:` prefix of a planner's `<Settings>` names.
const PLANNER_NAMESPACE: &str = "http://v8.1c.ru/8.3/data/planner";
/// The chart namespace the direct children of `<pl:timeScale>` and the scale
/// level itself declare inline.
const CHART_NAMESPACE: &str = "http://v8.1c.ru/8.2/data/chart";
/// The value type id a planner stores ahead of its record.
const PLANNER_VALUE_TYPE_UUID: &str = "43dc7f37-5b1d-42a7-8f28-f545080d0255";

/// The time unit shared by `<pl:periodicVariantUnit>` and a scale level's
/// `<measure>`.
const TIME_UNITS: &[(&str, &str)] = &[
    ("Second", "5"),
    ("Minute", "10"),
    ("Hour", "20"),
    ("Day", "30"),
    ("Week", "40"),
    ("Month", "50"),
    ("Quarter", "60"),
    ("Year", "70"),
];
/// The `BWAValue` header switches: a boolean with an `auto` state of its own.
const BWA_VALUES: &[(&str, &str)] = &[("false", "0"), ("true", "1"), ("auto", "2")];
const BORDER_STYLES: &[(&str, &str)] =
    &[("WithoutBorder", "0"), ("Single", "1"), ("Double", "200")];
const LINE_STYLES: &[(&str, &str)] = &[
    ("None", "0"),
    ("Solid", "1"),
    ("Dotted", "2"),
    ("Dashed", "3"),
    ("DashDotted", "4"),
];

/// `<Settings xsi:type="pl:Planner">`, the whole element as Form.xml spells
/// it, to the member-14 value `{0,1,"Planner",{…}}`.
pub(crate) fn format_form_embedded_planner(settings_xml: &str) -> Result<String> {
    let root = parse_xml(settings_xml)?;
    ensure!(
        root.name == "Settings"
            && attributes_are(
                &root,
                &[("xmlns:pl", PLANNER_NAMESPACE), ("xsi:type", "pl:Planner")]
            ),
        "the attribute's <Settings> is not a planner the writer reads"
    );
    let record = planner_record(&root)?;
    let value = format!("{{0,1,\"Planner\",{{\"#\",{PLANNER_VALUE_TYPE_UUID},{record}}}}}");
    verify_round_trip(
        settings_xml,
        &value,
        crate::mssql_dump::render_form_planner_settings_value,
    )?;
    Ok(value)
}

/// `{8,<item count>,<item>*,0,<borderColor>,<textColor>,<backColor>,
/// <lineColor>,<font>,<begin>,<end>,<3 flags>,<wrap format>,<unit>,
/// <repetition>,<2 indents>,<time scale>,1,<period>,0,<displayCurrentDate>,
/// <itemsTimeRepresentation>,<behaviour>,<2 auto-min flags>,<2 minima>,
/// <2 BWA headers>,<border>,<newItemsTextType>}`.
///
/// The stored colour pair is text-then-back where the platform writes
/// back-then-text, in the planner block and in each item.
fn planner_record(root: &XmlNode) -> Result<String> {
    let mut c = Cursor::new(root, "pl:")?;
    let mut items = Vec::new();
    while let Some(item) = c.optional("item") {
        items.push(planner_item(item)?);
    }
    let border_color = color(c.required("borderColor")?)?;
    let back_color = color(c.required("backColor")?)?;
    let text_color = color(c.required("textColor")?)?;
    let line_color = color(c.required("lineColor")?)?;
    let planner_font = font(c.required("font")?)?;
    let begin = date(c.required("beginOfRepresentationPeriod")?)?;
    let end = date(c.required("endOfRepresentationPeriod")?)?;
    let align = boolean(c.required("alignElementsOfTimeScale")?)?;
    let display_scale_wrap = boolean(c.required("displayTimeScaleWrapHeaders")?)?;
    let display_wrap = boolean(c.required("displayWrapHeaders")?)?;
    let wrap_format = localized(c.required("timeScaleWrapHeadersFormat")?)?;
    let unit = code(c.required("periodicVariantUnit")?, TIME_UNITS)?;
    let repetition = integer(c.required("periodicVariantRepetition")?)?;
    let begin_indent = integer(c.required("timeScaleWrapBeginIndent")?)?;
    let end_indent = integer(c.required("timeScaleWrapEndIndent")?)?;
    let scale = time_scale(c.required("timeScale")?)?;
    let period = period(c.required("period")?)?;
    let display_current_date = boolean(c.required("displayCurrentDate")?)?;
    let items_time = code(
        c.required("itemsTimeRepresentation")?,
        &[
            ("DontDisplay", "0"),
            ("BeginTime", "1"),
            ("BeginAndEndTime", "2"),
        ],
    )?;
    let behaviour = code(
        c.required("itemsBehaviorWhenSpaceInsufficient")?,
        &[("CollapseItems", "0")],
    )?;
    let auto_min_column = boolean(c.required("autoMinColumnWidth")?)?;
    let auto_min_row = boolean(c.required("autoMinRowHeight")?)?;
    let min_column = integer(c.required("minColumnWidth")?)?;
    let min_row = integer(c.required("minRowHeight")?)?;
    let fix_dimensions = code(c.required("fixDimensionsHeader")?, BWA_VALUES)?;
    let fix_scale = code(c.required("fixTimeScaleHeader")?, BWA_VALUES)?;
    let border = planner_border(c.required("border")?)?;
    let new_items_text = code(
        c.required("newItemsTextType")?,
        &[("String", "0"), ("FormattedString", "1")],
    )?;
    c.finish()?;

    let mut record = format!("{{8,{}", items.len());
    for item in &items {
        record.push(',');
        record.push_str(item);
    }
    record.push_str(&format!(
        ",0,{border_color},{text_color},{back_color},{line_color},{planner_font},{begin},{end},\
         {align},{display_scale_wrap},{display_wrap},{wrap_format},{unit},{repetition},\
         {begin_indent},{end_indent},{scale},1,{period},0,{display_current_date},{items_time},\
         {behaviour},{auto_min_column},{auto_min_row},{min_column},{min_row},{fix_dimensions},\
         {fix_scale},{border},{new_items_text}}}"
    ));
    Ok(record)
}

/// One `<pl:item>`: `{3,{"U"},<empty picture>,<begin>,<end>,<text>,
/// <borderColor>,<textColor>,<backColor>,<font>,<deleted>,<replacementDate>,
/// {0},0,0,<tooltip>,<id>,<border>,0,<textFormatted>,<editMode>}`.
fn planner_item(node: &XmlNode) -> Result<String> {
    let mut c = Cursor::new(node, "pl:")?;
    let value = c.required("value")?;
    ensure!(
        attributes_are(value, &[("xsi:nil", "true")])
            && value.children.is_empty()
            && value.text.trim().is_empty(),
        "<pl:value> carries a value the planner writer has not measured"
    );
    let text = string(c.required("text")?)?;
    let tooltip = string(c.required("tooltip")?)?;
    let begin = date(c.required("begin")?)?;
    let end = date(c.required("end")?)?;
    let border_color = color(c.required("borderColor")?)?;
    let back_color = color(c.required("backColor")?)?;
    let text_color = color(c.required("textColor")?)?;
    let item_font = font(c.required("font")?)?;
    let dimension_values = c.required("dimensionValues")?;
    ensure!(
        leaf(dimension_values)?.trim().is_empty(),
        "<pl:dimensionValues> carries a value the planner writer has not measured"
    );
    let replacement = date(c.required("replacementDate")?)?;
    let deleted = boolean(c.required("deleted")?)?;
    let id = uuid(c.required("id")?)?;
    let text_formatted = boolean(c.required("textFormatted")?)?;
    let border = planner_border(c.required("border")?)?;
    let edit_mode = code(
        c.required("editMode")?,
        &[("DisableEdit", "0"), ("EnableEdit", "3")],
    )?;
    c.finish()?;
    Ok(format!(
        "{{3,{{\"U\"}},{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{begin},{end},{text},{border_color},\
         {text_color},{back_color},{item_font},{deleted},{replacement},{{0}},0,0,{tooltip},{id},\
         {border},0,{text_formatted},{edit_mode}}}"
    ))
}

/// `<pl:timeScale>`: `{3,<placement>,<level count>,<level>*,<transparent>,
/// <backColor>,<textColor>,<currentLevel>}`. Its direct children sit in the
/// chart namespace, which the platform spells inline on each of them.
fn time_scale(node: &XmlNode) -> Result<String> {
    let mut c = Cursor::new(node, "")?;
    let placement = code(
        &chart_child(c.required("placement")?)?,
        &[("Top", "0"), ("Bottom", "1"), ("Left", "2"), ("Right", "3")],
    )?;
    let mut levels = Vec::new();
    while let Some(level) = c.optional("level") {
        levels.push(time_scale_level(&chart_child(level)?)?);
    }
    let transparent = boolean(&chart_child(c.required("transparent")?)?)?;
    let back_color = color(&chart_child(c.required("backColor")?)?)?;
    let text_color = color(&chart_child(c.required("textColor")?)?)?;
    let current_level = integer(&chart_child(c.required("currentLevel")?)?)?;
    c.finish()?;
    let mut record = format!("{{3,{placement},{}", levels.len());
    for level in &levels {
        record.push(',');
        record.push_str(level);
    }
    record.push_str(&format!(
        ",{transparent},{back_color},{text_color},{current_level}}}"
    ));
    Ok(record)
}

/// One `<level>` of the scale: `{8,<measure>,<interval>,<show>,<line>,
/// <scaleColor>,<dayFormatRule>,<format>,{0,{1,0,<ticks>}},<backColor>,
/// <textColor>,<showPereodicalLabels>}`. Its own children inherit the chart
/// namespace and carry no attribute.
fn time_scale_level(node: &XmlNode) -> Result<String> {
    let mut c = Cursor::new(node, "")?;
    let measure = code(c.required("measure")?, TIME_UNITS)?;
    let interval = integer(c.required("interval")?)?;
    let show = boolean(c.required("show")?)?;
    let line = scale_line(c.required("line")?)?;
    let scale_color = color(c.required("scaleColor")?)?;
    let day_rule = code(
        c.required("dayFormatRule")?,
        &[
            ("MonthDay", "1"),
            ("WeekDay", "2"),
            ("MonthDayWeekDay", "3"),
        ],
    )?;
    let format = localized(c.required("format")?)?;
    let labels = c.required("labels")?;
    let mut l = Cursor::new(labels, "")?;
    // Dated labels ahead of the ticks: `(<key>,{5,{0},{1,0},{"U"},<lineColor>,
    // <textColor>,{1,{1,0},0}})` each, the shape the exporter's
    // `form_planner_labels_xml` reads (empty unformatted text only).
    // Управление задачами `Reports/узПланированиеПроекта/Forms/
    // ФормаУправляемая` carries two.
    let mut label_records = Vec::new();
    while let Some(label) = l.optional("label") {
        let mut f = Cursor::new(label, "")?;
        let key = date(f.required("key")?)?;
        let text = f.required("text")?;
        ensure!(
            text.children.is_empty() && text.text.trim().is_empty(),
            "a planner scale label with text is not measured"
        );
        ensure!(
            leaf(f.required("textFormatted")?)?.trim() == "false",
            "a formatted planner scale label is not measured"
        );
        let line_color = color(f.required("lineColor")?)?;
        let text_color = color(f.required("textColor")?)?;
        f.finish()?;
        label_records.push(format!(
            "{key},{{5,{{0}},{{1,0}},{{\"U\"}},{line_color},{text_color},{{1,{{1,0}},0}}}}"
        ));
    }
    let ticks = integer(l.required("ticks")?)?;
    l.finish()?;
    let labels = if label_records.is_empty() {
        format!("{{0,{{1,0,{ticks}}}}}")
    } else {
        format!(
            "{{0,{{1,{},{},{ticks}}}}}",
            label_records.len(),
            label_records.join(",")
        )
    };
    let back_color = color(c.required("backColor")?)?;
    let text_color = color(c.required("textColor")?)?;
    let show_periodical = boolean(c.required("showPereodicalLabels")?)?;
    c.finish()?;
    Ok(format!(
        "{{8,{measure},{interval},{show},{line},{scale_color},{day_rule},{format},\
         {labels},{back_color},{text_color},{show_periodical}}}"
    ))
}

/// `<line width="W" gap="G"><v8ui:style xsi:type="v8ui:ChartLineType">…` to
/// `{4,0,{0},<style>,<width>,0,<line uuid>,<gap>}`: unlike the chart family's
/// line, the scale's varies both its style and its gap.
fn scale_line(node: &XmlNode) -> Result<String> {
    ensure!(
        node.attributes.len() == 2,
        "<{}> names line attributes the planner writer has not measured",
        node.name
    );
    let width = width_of(node)?;
    let gap = match node.attribute("gap") {
        Some("false") => "0",
        Some("true") => "1",
        _ => bail!(
            "<{}> names a gap the planner writer has not measured",
            node.name
        ),
    };
    let spelled = style_of(node, "v8ui:ChartLineType")?;
    let style = LINE_STYLES
        .iter()
        .find_map(|(name, style)| (*name == spelled).then_some(*style))
        .ok_or_else(|| {
            anyhow!(
                "<{}> names the line style {spelled}, which the planner writer has not measured",
                node.name
            )
        })?;
    Ok(format!("{{4,0,{{0}},{style},{width},0,{LINE_UUID},{gap}}}"))
}

/// `<pl:border width="W"><v8ui:style xsi:type="v8ui:ControlBorderType">…` to
/// `{3,0,{0},<style>,<width>,0,<border uuid>}`.
fn planner_border(node: &XmlNode) -> Result<String> {
    ensure!(
        node.attributes.len() == 1,
        "<{}> names border attributes the planner writer has not measured",
        node.name
    );
    let width = width_of(node)?;
    let spelled = style_of(node, "v8ui:ControlBorderType")?;
    let style = BORDER_STYLES
        .iter()
        .find_map(|(name, style)| (*name == spelled).then_some(*style))
        .ok_or_else(|| {
            anyhow!(
                "<{}> names the border style {spelled}, which the planner writer has not measured",
                node.name
            )
        })?;
    Ok(format!("{{3,0,{{0}},{style},{width},0,{BORDER_UUID}}}"))
}

/// `<pl:period>`: `{1,<begin>,<end>,0}`.
fn period(node: &XmlNode) -> Result<String> {
    let mut c = Cursor::new(node, "pl:")?;
    let begin = date(c.required("begin")?)?;
    let end = date(c.required("end")?)?;
    c.finish()?;
    Ok(format!("{{1,{begin},{end},0}}"))
}

/// `YYYY-MM-DDTHH:MM:SS` to the stored `YYYYMMDDHHMMSS`.
fn date(node: &XmlNode) -> Result<String> {
    let text = leaf(node)?.trim();
    let bytes = text.as_bytes();
    let shaped = bytes.len() == 19
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            _ => byte.is_ascii_digit(),
        });
    ensure!(
        shaped,
        "<{}> spells {text}, which is not a date the planner writer places",
        node.name
    );
    Ok(text
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(char::from)
        .collect())
}

fn uuid(node: &XmlNode) -> Result<String> {
    let text = leaf(node)?.trim();
    ensure!(
        text.len() == 36
            && text.bytes().enumerate().all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => byte == b'-',
                _ => byte.is_ascii_hexdigit(),
            }),
        "<{}> spells {text}, which is not a uuid",
        node.name
    );
    Ok(text.to_string())
}

/// A direct child of `<pl:timeScale>`, or the scale level itself: the one
/// attribute it carries is the inline chart namespace, which the leaf and
/// record readers do not expect.
fn chart_child(node: &XmlNode) -> Result<XmlNode> {
    ensure!(
        attributes_are(node, &[("xmlns", CHART_NAMESPACE)]),
        "<{}> is not in the chart namespace the planner writer reads",
        node.name
    );
    Ok(XmlNode {
        attributes: Vec::new(),
        ..node.clone()
    })
}

/// The child elements of one element, consumed in the order the decoder
/// writes them, each name taken under `prefix`.
struct Cursor<'a> {
    owner: &'a str,
    nodes: &'a [XmlNode],
    next: usize,
    prefix: &'static str,
}

impl<'a> Cursor<'a> {
    fn new(node: &'a XmlNode, prefix: &'static str) -> Result<Self> {
        ensure!(
            node.text.trim().is_empty(),
            "<{}> mixes text with its elements",
            node.name
        );
        Ok(Self {
            owner: &node.name,
            nodes: &node.children,
            next: 0,
            prefix,
        })
    }

    fn optional(&mut self, name: &str) -> Option<&'a XmlNode> {
        let node = self.nodes.get(self.next)?;
        (node.name.strip_prefix(self.prefix) == Some(name)).then(|| {
            self.next += 1;
            node
        })
    }

    fn required(&mut self, name: &str) -> Result<&'a XmlNode> {
        match self.optional(name) {
            Some(node) => Ok(node),
            None => match self.nodes.get(self.next) {
                Some(found) => Err(anyhow!(
                    "<{}> spells <{}> where the planner writer expects <{}{name}>",
                    self.owner,
                    found.name,
                    self.prefix
                )),
                None => Err(anyhow!(
                    "<{}> ends before <{}{name}>",
                    self.owner,
                    self.prefix
                )),
            },
        }
    }

    fn finish(&self) -> Result<()> {
        match self.nodes.get(self.next) {
            Some(extra) => Err(anyhow!(
                "<{}> names <{}>, which the planner writer cannot place",
                self.owner,
                extra.name
            )),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &[(&str, &str)] = &[
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/control.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/control-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/distinct-values.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/distinct-values-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/bits-a.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/bits-a-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/bits-b.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/bits-b-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/bits-c.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/bits-c-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/bits-d.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/bits-d-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/font-item.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/font-item-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/font-planner.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/font-planner-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/items-two.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/items-two-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/items-zero.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/items-zero-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/native-do-bron.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/native-do-bron-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/native-do-meropriyatiya.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/native-do-meropriyatiya-settings.xml"
            ),
        ),
        (
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/raw/native-do-vremya.txt"
            ),
            include_str!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/form-planner-settings/native/native-do-vremya-settings.xml"
            ),
        ),
    ];

    /// Every platform-proven record of the fixture -- the ten seeds the
    /// platform loaded from XML and the three native Документооборот
    /// attributes -- is rebuilt byte for byte from its native `<Settings>`.
    #[test]
    fn rebuilds_every_platform_proven_planner_record() {
        assert_eq!(FIXTURES.len(), 13);
        for (raw, native) in FIXTURES {
            let value = format_form_embedded_planner(native.trim_end_matches(['\r', '\n']))
                .unwrap_or_else(|error| panic!("{error:#}\n{native}"));
            assert_eq!(value, raw.trim(), "\n{native}");
        }
    }

    /// An element the decoder would not have written is refused, not guessed.
    #[test]
    fn refuses_what_the_decoder_does_not_publish() {
        let (_, native) = FIXTURES[0];
        let moved = native.replace(
            "<pl:itemsBehaviorWhenSpaceInsufficient>CollapseItems",
            "<pl:itemsBehaviorWhenSpaceInsufficient>Other",
        );
        let error = format_form_embedded_planner(&moved).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("itemsBehaviorWhenSpaceInsufficient"),
            "{error:#}"
        );
        let reordered = native.replacen(
            "<pl:displayCurrentDate>true</pl:displayCurrentDate>\r\n",
            "",
            1,
        );
        let error = format_form_embedded_planner(&reordered).unwrap_err();
        assert!(
            error.to_string().contains("displayCurrentDate"),
            "{error:#}"
        );
    }
}
