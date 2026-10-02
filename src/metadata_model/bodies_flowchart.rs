//! Business process flowcharts and graphical-schema templates, base-free:
//! `Ext/Flowchart.xml` -> the `<uuid>.7` row, a GraphicalSchema template's
//! `Ext/Template.xml` (with the pictures under `Ext/Template/Items`) -> its
//! `<uuid>.0` row. Both are stored in one grammar.
//!
//! `{5,{<scheme>},N,<code>,<item>...,N}`: the scheme (background, grid, print
//! parameters), then each item as its kind code and record, in document
//! order, then the item count again. The grammar is the one the exporter
//! reads (`mssql_dump::parse_business_process_flowchart_text_with_types`);
//! what the XML does not spell -- the outline points of a shape, the kind
//! constants of each record, the true-branch mark of a line -- is measured
//! over the 22 flowcharts of the four corpora:
//!
//! - a shape's outline is computed from its `<Location>`: a rectangle for
//!   activities, processings and sub-processes; a pentagon for start and
//!   completion whose point rises `w/2·tan 30°`; a hexagon for a condition
//!   whose ends rise `h/2·tan 30°`; triangles for split and join;
//!   a group activity's rectangle stops 4 short of its right and bottom;
//!   a start or completion whose point would not fit rises half its height;
//! - a line records which branch it leaves: 1 from a condition's true port
//!   (and from its right port, 3, unless that is the false port), the case
//!   row from a switch (`(port - 6) / 2`), 0 otherwise;
//! - the addressing attributes of an activity are ordered by uuid;
//! - an activity's record ends `{3,2,0}` in a business process and
//!   `{3,16,1}` in a template (every activity of the corpora);
//! - an inline picture (`<Abs>`) is the file beside the scheme, stored as
//!   base64 in lines of 64 separated by CR CR LF.
//!
//! The record closes with a counter the XML does not carry: the item count
//! for a scheme never edited after deletions (18 of 20 flowcharts, 29 of 64
//! templates of ERP УХ); the others count deleted items too.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use super::DescriptorContext;
use super::brace::{Brace, NIL_UUID, parse_row};
use super::common::{stores_layout_8_5_1, up_convert_primitives_8_5_1};
use super::types::{design_time_ref, object_uuid};
use super::xml::{Element, parse_element_tree_raw_line_breaks};
use crate::brace_list;
use crate::compiler::bodies::form_native::{format_native_color, format_native_font};

/// The border and line record's constant member.
const LINE_RECORD_UUID: &str = "e45c0cd8-a878-4bcb-8e1a-af934481e1cc";

/// `Ext/Flowchart.xml` next to the business process's XML.
pub fn flowchart_path(owner_xml: &Path) -> PathBuf {
    owner_xml
        .with_extension("")
        .join("Ext")
        .join("Flowchart.xml")
}

/// The `<uuid>.7` row, `None` when the process has no `Ext/Flowchart.xml`.
pub fn flowchart_row(owner_xml: &Path, context: &DescriptorContext) -> Result<Option<Brace>> {
    scheme_row(
        &flowchart_path(owner_xml),
        SchemeOwner::BusinessProcess,
        context,
    )
}

/// A flowchart's strings as stored: every LF becomes CR LF, even one after a
/// CR. The exporter drops one CR before each LF of a flowchart string, so a
/// value holding CR LF itself is dumped CR LF and stored CR CR LF
/// (Библиотека стандартных подсистем, `BusinessProcesses/Задание`: CR LF in
/// the XML, CR CR LF stored), where a descriptor string is dumped as stored
/// (`common::crlf_strings`).
fn flowchart_strings(node: &mut Brace) {
    match node {
        Brace::Str(value) if value.contains('\n') => *value = value.replace('\n', "\r\n"),
        Brace::List(items) => items.iter_mut().for_each(flowchart_strings),
        _ => {}
    }
}

/// `Ext/Template.xml` next to a template's XML.
pub fn template_body_path(template_xml: &Path) -> PathBuf {
    template_xml
        .with_extension("")
        .join("Ext")
        .join("Template.xml")
}

/// Whether a template's XML declares a graphical schema.
pub fn is_graphical_schema_template(template: &Element) -> bool {
    template
        .path(&["Properties", "TemplateType"])
        .is_some_and(|kind| kind.text.trim() == "GraphicalSchema")
}

/// A GraphicalSchema template's `<uuid>.0` row, `None` without a body.
pub fn graphical_schema_row(
    template_xml: &Path,
    context: &DescriptorContext,
) -> Result<Option<Brace>> {
    scheme_row(
        &template_body_path(template_xml),
        SchemeOwner::Template,
        context,
    )
}

/// Who owns a scheme: the one thing that changes its records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SchemeOwner {
    BusinessProcess,
    Template,
}

fn scheme_row(
    path: &Path,
    owner: SchemeOwner,
    context: &DescriptorContext,
) -> Result<Option<Brace>> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let schema = parse_element_tree_raw_line_breaks(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let pictures = path.with_extension("").join("Items");
    let mut row = Flowchart::new(&schema, context, owner, pictures)?.to_brace()?;
    flowchart_strings(&mut row);
    if stores_layout_8_5_1(context) {
        up_convert_primitives_8_5_1(&mut row);
    }
    Ok(Some(row))
}

struct Flowchart<'a> {
    schema: &'a Element,
    items: Vec<&'a Element>,
    context: &'a DescriptorContext,
    owner: SchemeOwner,
    /// `<scheme stem>/Items`: the inline pictures, one folder per item.
    pictures: PathBuf,
}

fn text<'a>(element: &'a Element, name: &str) -> &'a str {
    element.child_text(name).unwrap_or_default()
}

fn number(text: &str, what: &str) -> Result<i64> {
    let text = text.trim();
    text.parse::<i64>()
        .with_context(|| format!("{what} is not a number: {text:?}"))
}

fn flag(element: &Element, name: &str) -> bool {
    text(element, name).trim() == "true"
}

fn code(table: &[(&str, i64)], value: &str, what: &str) -> Result<i64> {
    let value = value.trim();
    table
        .iter()
        .find_map(|(name, code)| (*name == value).then_some(*code))
        .ok_or_else(|| anyhow!("no flowchart code for {what} {value:?}"))
}

fn literal(text: &str) -> Result<Brace> {
    parse_row(text.as_bytes()).with_context(|| format!("bad brace literal {text:?}"))
}

/// `{1,<count>,{"lang","text"}...}`.
fn localized(element: Option<&Element>) -> Brace {
    let pairs = element.map(Element::localized).unwrap_or_default();
    let mut items = vec![Brace::num(1), Brace::num(pairs.len() as i64)];
    for (lang, content) in pairs {
        items.push(brace_list![Brace::str(lang), Brace::str(content)]);
    }
    Brace::List(items)
}

const DRAW_GRID_MODES: &[(&str, i64)] = &[("None", 0), ("Dots", 1), ("Lines", 3)];
const FIT_PAGE_MODES: &[(&str, i64)] = &[("Auto", 0)];
const HORIZONTAL_ALIGNS: &[(&str, i64)] = &[("Left", 0), ("Center", 1), ("Right", 2)];
const VERTICAL_ALIGNS: &[(&str, i64)] = &[("Top", 0), ("Center", 1), ("Bottom", 2)];
const PICTURE_LOCATIONS: &[(&str, i64)] = &[("Left", 1), ("Top", 2), ("Center", 5)];
const PICTURE_SIZES: &[(&str, i64)] = &[("RealSize", 0), ("Proportionally", 2), ("AutoSize", 4)];
const LINE_STYLES: &[(&str, i64)] = &[("None", 0), ("Solid", 1), ("Dashed", 2), ("Dotted", 3)];
const TEXT_LOCATIONS: &[(&str, i64)] = &[("FirstSegment", 0), ("Middle", 1)];
const ARROWS: &[(&str, i64)] = &[("None", 0), ("Filled", 1), ("Blank", 2)];
const SHAPES: &[(&str, i64)] = &[
    ("None", 0),
    ("Document", 10),
    ("Block", 11),
    ("HorizontalBrackets", 12),
];

impl<'a> Flowchart<'a> {
    fn new(
        schema: &'a Element,
        context: &'a DescriptorContext,
        owner: SchemeOwner,
        pictures: PathBuf,
    ) -> Result<Self> {
        let items = schema
            .child("Items")
            .map(|items| items.children.iter().collect())
            .unwrap_or_default();
        Ok(Self {
            schema,
            items,
            context,
            owner,
            pictures,
        })
    }

    fn to_brace(&self) -> Result<Brace> {
        let mut top = vec![
            Brace::num(5),
            brace_list![self.scheme()?],
            Brace::num(self.items.len() as i64),
        ];
        for item in &self.items {
            let (kind_code, body) = self.item(item)?;
            top.push(Brace::num(kind_code));
            top.push(body);
        }
        top.push(Brace::num(self.items.len() as i64));
        Ok(Brace::List(top))
    }

    /// `{1,<back>,<grid>,<h step>,<v step>,<grid mode>,6,6,{"N",top},7,
    /// {"N",left},8,{"N",bottom},9,{"N",right},13,{"N",b&w},16,{"N",fit}}`.
    fn scheme(&self) -> Result<Brace> {
        let schema = self.schema;
        let print = schema
            .child("PrintParameters")
            .ok_or_else(|| anyhow!("flowchart without <PrintParameters>"))?;
        let parameter = |key: i64, value: i64| {
            [
                Brace::num(key),
                brace_list![Brace::str("N"), Brace::num(value)],
            ]
        };
        let mut items = vec![
            Brace::num(1),
            self.color(text(schema, "BackColor"))?,
            Brace::flag(flag(schema, "GridEnabled")),
            Brace::num(number(
                text(schema, "GridHorizontalStep"),
                "GridHorizontalStep",
            )?),
            Brace::num(number(
                text(schema, "GridVerticalStep"),
                "GridVerticalStep",
            )?),
            Brace::num(code(
                DRAW_GRID_MODES,
                text(schema, "DrawGridMode"),
                "DrawGridMode",
            )?),
            Brace::num(6),
        ];
        items.extend(parameter(6, number(text(print, "TopMargin"), "TopMargin")?));
        items.extend(parameter(
            7,
            number(text(print, "LeftMargin"), "LeftMargin")?,
        ));
        items.extend(parameter(
            8,
            number(text(print, "BottomMargin"), "BottomMargin")?,
        ));
        items.extend(parameter(
            9,
            number(text(print, "RightMargin"), "RightMargin")?,
        ));
        items.extend(parameter(13, i64::from(flag(print, "BlackAndWhite"))));
        items.extend(parameter(
            16,
            code(FIT_PAGE_MODES, text(print, "FitPageMode"), "FitPageMode")?,
        ));
        Ok(Brace::List(items))
    }

    /// A colour: `auto` is the unset `{3,4,{0}}`, the rest as a form body
    /// stores it.
    fn color(&self, spelled: &str) -> Result<Brace> {
        let spelled = spelled.trim();
        if spelled == "auto" || spelled.is_empty() {
            return literal("{3,4,{0}}");
        }
        let native = format_native_color(Some(spelled), |name| {
            self.context
                .index
                .uuid_of(&format!("StyleItem.{name}"))
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow!("unsupported flowchart colour {spelled:?}"))?;
        literal(&native)
    }

    fn font(&self, element: Option<&Element>) -> Result<Brace> {
        let attributes = element
            .map(|font| {
                font.attrs
                    .iter()
                    .cloned()
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let native = format_native_font(&attributes, |name| {
            self.context
                .index
                .uuid_of(&format!("StyleItem.{name}"))
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow!("unsupported flowchart font {attributes:?}"))?;
        literal(&native)
    }

    /// `{7,<back>,<line>,<text>,<font>,<tooltip>,<h align>,<v align>,
    /// <picture location>,<z order>,<hyperlink>,<transparent>,0,0}`.
    fn style(&self, properties: &Element) -> Result<Brace> {
        Ok(brace_list![
            Brace::num(7),
            self.color(text(properties, "BackColor"))?,
            self.color(text(properties, "LineColor"))?,
            self.color(text(properties, "TextColor"))?,
            self.font(properties.child("Font"))?,
            localized(properties.child("ToolTip")),
            Brace::num(code(
                HORIZONTAL_ALIGNS,
                text(properties, "HorizontalAlign"),
                "HorizontalAlign"
            )?),
            Brace::num(code(
                VERTICAL_ALIGNS,
                text(properties, "VerticalAlign"),
                "VerticalAlign"
            )?),
            Brace::num(code(
                PICTURE_LOCATIONS,
                text(properties, "PictureLocation"),
                "PictureLocation"
            )?),
            Brace::num(number(text(properties, "ZOrder"), "ZOrder")?),
            Brace::flag(flag(properties, "Hyperlink")),
            Brace::flag(flag(properties, "Transparent")),
            Brace::num(0),
            Brace::num(0),
        ])
    }

    /// `{4,0,{0},<style>,<width>,<gap>,e45c0cd8-...,0}`: a shape's border or a
    /// line's line.
    fn line_record(&self, element: Option<&Element>) -> Result<Brace> {
        let (style, width, gap) = match element {
            Some(element) => (
                code(
                    LINE_STYLES,
                    element.child_text("style").unwrap_or("Solid"),
                    "line style",
                )?,
                number(element.attr("width").unwrap_or("1"), "line width")?,
                element.attr("gap") == Some("true"),
            ),
            None => (1, 1, false),
        };
        Ok(brace_list![
            Brace::num(4),
            Brace::num(0),
            brace_list![Brace::num(0)],
            Brace::num(style),
            Brace::num(width),
            Brace::flag(gap),
            Brace::atom(LINE_RECORD_UUID),
            Brace::num(0),
        ])
    }

    /// The picture record: empty, a platform picture by uuid (nine
    /// members), or an inline picture (ten members, the file's bytes as
    /// base64).
    fn picture(&self, element: Option<&Element>, item_name: &str) -> Result<Brace> {
        let reference = element
            .and_then(|picture| picture.child_text("Ref"))
            .map(str::trim)
            .unwrap_or_default();
        let inline = element
            .and_then(|picture| picture.child_text("Abs"))
            .map(str::trim)
            .unwrap_or_default();
        if reference.is_empty() && inline.is_empty() {
            return literal("{4,0,{0},\"\",-1,-1,1,0,\"\"}");
        }
        let load_transparent = element
            .and_then(|picture| picture.child_text("LoadTransparent"))
            .map(str::trim);
        let pixel = element.and_then(|picture| picture.child("TransparentPixel"));
        let coordinate = |name: &str| -> Result<i64> {
            match pixel.and_then(|pixel| pixel.attr(name)) {
                Some(value) => number(value, name),
                None => Ok(-1),
            }
        };
        let (x, y) = (coordinate("x")?, coordinate("y")?);
        if !inline.is_empty() {
            let path = self.pictures.join(item_name).join(inline);
            let bytes = fs::read(&path)
                .with_context(|| format!("failed to read picture {}", path.display()))?;
            return Ok(brace_list![
                Brace::num(4),
                Brace::num(3),
                brace_list![Brace::num(0)],
                Brace::str(""),
                Brace::num(x),
                Brace::num(y),
                Brace::flag(load_transparent == Some("true")),
                brace_list![brace_list![Brace::atom(base64_lines(&bytes))]],
                Brace::num(0),
                Brace::str(""),
            ]);
        }
        let uuid = crate::mssql_dump::standard_picture_uuid(reference)
            .ok_or_else(|| anyhow!("unsupported flowchart picture {reference}"))?;
        Ok(brace_list![
            Brace::num(4),
            Brace::num(1),
            brace_list![Brace::num(0), Brace::uuid(uuid)],
            Brace::str(""),
            Brace::num(x),
            Brace::num(y),
            Brace::flag(load_transparent != Some("false")),
            Brace::num(0),
            Brace::str(""),
        ])
    }

    fn location(properties: &Element) -> Result<(i64, i64, i64, i64)> {
        let location = properties
            .child("Location")
            .ok_or_else(|| anyhow!("flowchart shape without <Location>"))?;
        let value = |name: &str| number(location.attr(name).unwrap_or("0"), name);
        Ok((
            value("left")?,
            value("top")?,
            value("right")?,
            value("bottom")?,
        ))
    }

    /// `{<style>,5,l,t,r,b,<n>,<points>...,<picture size>,<picture>,<border>}`.
    fn shape_geometry(&self, tag: &str, properties: &Element) -> Result<Brace> {
        let (left, top, right, bottom) = Self::location(properties)?;
        let group = tag == "Activity" && flag(properties, "Group");
        let points = outline(tag, group, left, top, right, bottom)?;
        let mut items = vec![
            self.style(properties)?,
            Brace::num(5),
            Brace::num(left),
            Brace::num(top),
            Brace::num(right),
            Brace::num(bottom),
            Brace::num(points.len() as i64),
        ];
        for (x, y) in points {
            items.push(Brace::num(x));
            items.push(Brace::num(y));
        }
        items.push(Brace::num(code(
            PICTURE_SIZES,
            text(properties, "PictureSize"),
            "PictureSize",
        )?));
        items.push(self.picture(properties.child("Picture"), text(properties, "Name"))?);
        items.push(self.line_record(properties.child("Border"))?);
        Ok(Brace::List(items))
    }

    /// `{<code>,<record>}` of one item.
    fn item(&self, item: &Element) -> Result<(i64, Brace)> {
        let properties = item
            .child("Properties")
            .ok_or_else(|| anyhow!("flowchart item without <Properties>"))?;
        let id = number(item.attr("id").unwrap_or_default(), "item id")?;
        let base = brace_list![
            Brace::num(4),
            Brace::num(id),
            localized(properties.child("Title")),
            Brace::str(text(properties, "Name")),
            Brace::num(number(text(properties, "TabOrder"), "TabOrder")?),
        ];
        let head = || -> Result<Brace> {
            let uuid = item
                .attr("uuid")
                .ok_or_else(|| anyhow!("flowchart {} without uuid", item.name))?;
            Ok(brace_list![
                base.clone(),
                Brace::num(4),
                Brace::uuid(uuid),
                Brace::num(0)
            ])
        };
        let tag = item.name.as_str();
        let shape = |tail: Vec<Brace>| -> Result<Brace> {
            let mut wrapper = vec![self.shape_geometry(tag, properties)?];
            wrapper.extend(tail);
            Ok(brace_list![Brace::List(wrapper)])
        };
        Ok(match tag {
            "Decoration" => (
                0,
                brace_list![base.clone(), Brace::num(2), self.decoration(properties)?],
            ),
            "ConnectionLine" => (1, self.line(base.clone(), properties)?),
            "Start" => (
                2,
                brace_list![
                    head()?,
                    Brace::num(2),
                    shape(vec![Brace::num(1)])?,
                    events(item, &["BeforeStart"])
                ],
            ),
            "Completion" => (
                3,
                brace_list![
                    head()?,
                    Brace::num(2),
                    shape(vec![Brace::num(1)])?,
                    events(item, &["OnComplete"])
                ],
            ),
            "Condition" => (
                4,
                brace_list![
                    head()?,
                    Brace::num(1),
                    shape(vec![
                        Brace::num(3),
                        Brace::num(number(text(properties, "TruePortIndex"), "TruePortIndex")?),
                        Brace::num(number(
                            text(properties, "FalsePortIndex"),
                            "FalsePortIndex"
                        )?),
                    ])?,
                    events(item, &["ConditionCheck"])
                ],
            ),
            "Activity" => (
                5,
                brace_list![
                    head()?,
                    Brace::num(8),
                    shape(match self.owner {
                        SchemeOwner::BusinessProcess => {
                            vec![Brace::num(3), Brace::num(2), Brace::num(0)]
                        }
                        SchemeOwner::Template => {
                            vec![Brace::num(3), Brace::num(16), Brace::num(1)]
                        }
                    })?,
                    Brace::str(text(properties, "Explanation")),
                    Brace::flag(flag(properties, "Group")),
                    events(
                        item,
                        &[
                            "InteractiveActivationProcessing",
                            "BeforeCreateTasks",
                            "OnCreateTask",
                            "OnExecute",
                            "CheckExecutionProcessing",
                            "BeforeExecute",
                            "BeforeExecuteInteractively",
                        ]
                    ),
                    self.addressing_attributes(properties.child("AddressingAttributes"))?,
                    Brace::str(text(properties, "TaskDescription")),
                ],
            ),
            "Switch" => {
                let cases = properties.children_named("Case").collect::<Vec<_>>();
                let mut tail = vec![Brace::num(2), Brace::num(cases.len() as i64)];
                let mut record = vec![head()?, Brace::num(2)];
                for case in &cases {
                    tail.push(localized(case.child("description")));
                    tail.push(self.color(text(case, "backColor"))?);
                }
                record.push(shape(tail)?);
                record.push(Brace::num(cases.len() as i64));
                for case in &cases {
                    record.push(Brace::str(text(case, "name")));
                }
                record.push(events(item, &["SwitchProcessing"]));
                (6, Brace::List(record))
            }
            "Split" => (
                7,
                brace_list![head()?, Brace::num(1), shape(vec![Brace::num(1)])?],
            ),
            "Join" => (
                8,
                brace_list![head()?, Brace::num(1), shape(vec![Brace::num(1)])?],
            ),
            "Processing" => (
                9,
                brace_list![
                    head()?,
                    Brace::num(0),
                    shape(vec![Brace::num(1)])?,
                    events(item, &["Processing"])
                ],
            ),
            "SubBusinessProcess" => {
                let subprocess = text(properties, "Subprocess").trim();
                let subprocess = if subprocess.is_empty() {
                    NIL_UUID.to_string()
                } else {
                    object_uuid(subprocess, self.context)?
                };
                (
                    10,
                    brace_list![
                        head()?,
                        Brace::num(1),
                        shape(vec![Brace::num(1)])?,
                        events(
                            item,
                            &[
                                "BeforeCreateTasks",
                                "OnCreateTask",
                                "OnCreateSubBusinessProcesses",
                                "OnExecute",
                                "BeforeExecute",
                                "BeforeCreateSubBusinessProcesses",
                            ]
                        ),
                        Brace::uuid(&subprocess),
                        Brace::str(text(properties, "TaskDescription")),
                    ],
                )
            }
            other => bail!("unsupported flowchart item {other}"),
        })
    }

    /// `{{<style>,6,l,t,r,b,<picture size>,<picture>,<transparent>,<shape>,0,0}}`.
    fn decoration(&self, properties: &Element) -> Result<Brace> {
        let (left, top, right, bottom) = Self::location(properties)?;
        Ok(brace_list![brace_list![
            self.style(properties)?,
            Brace::num(6),
            Brace::num(left),
            Brace::num(top),
            Brace::num(right),
            Brace::num(bottom),
            Brace::num(code(
                PICTURE_SIZES,
                text(properties, "PictureSize"),
                "PictureSize"
            )?),
            self.picture(properties.child("Picture"), text(properties, "Name"))?,
            Brace::flag(flag(properties, "Transparent")),
            Brace::num(code(SHAPES, text(properties, "Shape"), "Shape")?),
            Brace::num(0),
            Brace::num(0),
        ]])
    }

    /// `{<base>,3,<from id>,<true branch>,<to id>,<decorative>,{{<style>,6,
    /// <n>,<points>...,<line>,<text location>,<from port>,<to port>,<m>,
    /// <segments>...,<begin arrow>,<end arrow>}}}`.
    fn line(&self, base: Brace, properties: &Element) -> Result<Brace> {
        let connect = properties.child("Connect");
        let end = |name: &str| -> Result<(i64, i64, String)> {
            let end = connect.and_then(|connect| connect.child(name));
            let item = end.map(|end| text(end, "Item").trim()).unwrap_or_default();
            let port = number(
                end.map(|end| text(end, "PortIndex")).unwrap_or("0"),
                "PortIndex",
            )?;
            let id = if item.is_empty() {
                -1
            } else {
                self.item_id(item)?
            };
            Ok((id, port, item.to_string()))
        };
        let (from_id, from_port, from_item) = end("From")?;
        let (to_id, to_port, _) = end("To")?;

        let mut geometry = vec![self.style(properties)?, Brace::num(6)];
        let points = properties
            .child("PivotPoints")
            .map(|points| points.children_named("Point").collect::<Vec<_>>())
            .unwrap_or_default();
        geometry.push(Brace::num(points.len() as i64));
        for point in points {
            geometry.push(Brace::num(number(point.attr("x").unwrap_or("0"), "x")?));
            geometry.push(Brace::num(number(point.attr("y").unwrap_or("0"), "y")?));
        }
        geometry.push(self.line_record(properties.child("Line"))?);
        geometry.push(Brace::num(code(
            TEXT_LOCATIONS,
            text(properties, "TextLocation"),
            "TextLocation",
        )?));
        geometry.push(Brace::num(from_port));
        geometry.push(Brace::num(to_port));
        let segments = properties
            .child("ManualyMovedSegments")
            .map(|segments| segments.children_named("Segment").collect::<Vec<_>>())
            .unwrap_or_default();
        geometry.push(Brace::num(segments.len() as i64));
        for segment in segments {
            for (name, axis) in [("Start", "x"), ("Start", "y"), ("End", "x"), ("End", "y")] {
                let point = segment
                    .child(name)
                    .ok_or_else(|| anyhow!("segment without <{name}>"))?;
                geometry.push(Brace::num(number(point.attr(axis).unwrap_or("0"), axis)?));
            }
            geometry.push(Brace::num(number(
                segment.attr("index").unwrap_or("0"),
                "segment index",
            )?));
        }
        geometry.push(Brace::num(code(
            ARROWS,
            text(properties, "BeginArrow"),
            "BeginArrow",
        )?));
        geometry.push(Brace::num(code(
            ARROWS,
            text(properties, "EndArrow"),
            "EndArrow",
        )?));

        Ok(brace_list![
            base,
            Brace::num(3),
            Brace::num(from_id),
            Brace::num(self.branch(&from_item, from_port)),
            Brace::num(to_id),
            Brace::flag(flag(properties, "DecorativeLine")),
            brace_list![Brace::List(geometry)],
        ])
    }

    /// The id of the item a line names.
    fn item_id(&self, name: &str) -> Result<i64> {
        let item = self
            .items
            .iter()
            .find(|item| {
                item.child("Properties")
                    .and_then(|properties| properties.child_text("Name"))
                    == Some(name)
            })
            .ok_or_else(|| anyhow!("a flowchart line names no item {name:?}"))?;
        number(item.attr("id").unwrap_or_default(), "item id")
    }

    /// The branch a line leaves its source by: from a condition 1 for its
    /// true port (and for the right port, 3, unless that is the false one),
    /// from a switch the case row of the port, 0 otherwise.
    fn branch(&self, from_item: &str, from_port: i64) -> i64 {
        let Some(source) = self.items.iter().find(|item| {
            item.child("Properties")
                .and_then(|properties| properties.child_text("Name"))
                == Some(from_item)
        }) else {
            return 0;
        };
        let Some(properties) = source.child("Properties") else {
            return 0;
        };
        match source.name.as_str() {
            "Condition" => {
                let port = |name: &str| text(properties, name).trim().parse::<i64>().ok();
                let true_port = port("TruePortIndex");
                let false_port = port("FalsePortIndex");
                i64::from(Some(from_port) == true_port || (from_port == 3 && false_port != Some(3)))
            }
            "Switch" if from_port >= 6 => (from_port - 6) / 2,
            _ => 0,
        }
    }

    /// `{<count>,{<attribute uuid>,<value>}...}`, ordered by uuid.
    fn addressing_attributes(&self, list: Option<&Element>) -> Result<Brace> {
        let mut attributes = Vec::new();
        for attribute in list
            .into_iter()
            .flat_map(|list| list.children_named("AddressingAttribute"))
        {
            let reference = attribute.attr("ref").unwrap_or_default();
            let uuid = object_uuid(reference, self.context)?;
            let value = match attribute.child("Value") {
                None => brace_list![Brace::str("U")],
                Some(value) if value.is_nil() => brace_list![Brace::str("U")],
                Some(value) => design_time_ref(&value.text, self.context)?,
            };
            attributes.push((uuid, value));
        }
        attributes.sort_by(|left, right| left.0.cmp(&right.0));
        let mut items = vec![Brace::num(attributes.len() as i64)];
        for (uuid, value) in attributes {
            items.push(brace_list![Brace::uuid(&uuid), value]);
        }
        Ok(Brace::List(items))
    }
}

/// `{<count>,{<index>,"<handler>"}...}`: the events that name a handler, by
/// their index in the kind's event list.
fn events(item: &Element, names: &[&str]) -> Brace {
    let mut handlers = Vec::new();
    for event in item
        .child("Events")
        .into_iter()
        .flat_map(|events| events.children_named("Event"))
    {
        let handler = event.text.trim();
        if handler.is_empty() {
            continue;
        }
        let Some(index) = names
            .iter()
            .position(|name| Some(*name) == event.attr("name"))
        else {
            continue;
        };
        handlers.push((index, handler.to_string()));
    }
    handlers.sort_by_key(|(index, _)| *index);
    let mut items = vec![Brace::num(handlers.len() as i64)];
    for (index, handler) in handlers {
        items.push(brace_list![Brace::num(index as i64), Brace::str(handler)]);
    }
    Brace::List(items)
}

/// `h/2·tan 30°` (or `w/2·tan 30°`), truncated: how far a pointed shape's
/// point rises.
fn rise(extent: i64) -> i64 {
    ((extent as f64) * (30f64.to_radians().tan()) / 2.0) as i64
}

/// A pentagon's point: `w/2·tan 30°`, or half the height when that does not
/// fit (a 150×38 completion rises 19, not 43).
fn roof(w: i64, h: i64) -> i64 {
    let roof = rise(w);
    if roof < h { roof } else { h / 2 }
}

/// A shape's outline as the platform stores it, from its location.
fn outline(
    tag: &str,
    group: bool,
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
) -> Result<Vec<(i64, i64)>> {
    let (w, h) = (right - left, bottom - top);
    let (r, b) = (right - 1, bottom - 1);
    Ok(match tag {
        "Activity" if group => vec![
            (left, top),
            (right - 4, top),
            (right - 4, bottom - 4),
            (left, bottom - 4),
        ],
        "Activity" | "Processing" | "SubBusinessProcess" | "Switch" => {
            vec![(left, top), (r, top), (r, b), (left, b)]
        }
        "Start" => {
            let shoulder = bottom - roof(w, h);
            vec![
                (left, top),
                (r, top),
                (r, shoulder),
                (left + w / 2, b),
                (left, shoulder),
            ]
        }
        "Completion" => {
            let shoulder = top + roof(w, h);
            vec![
                (left + w / 2, top),
                (r, shoulder),
                (r, b),
                (left, b),
                (left, shoulder),
            ]
        }
        "Condition" => {
            let inset = rise(h);
            let middle = top + h / 2;
            vec![
                (left, middle),
                (left + inset, top),
                (r - inset, top),
                (r, middle),
                (r - inset, b),
                (left + inset, b),
            ]
        }
        "Split" => vec![(left, top), (right - 2, top), (left + (w - 2) / 2, b)],
        "Join" => vec![(left, b), (right - 2, b), (left + (w - 2) / 2, top)],
        other => bail!("no outline for flowchart {other}"),
    })
}

/// Base64 as the platform stores a picture: `#base64:` and lines of 64,
/// each full line ended by CR CR LF (the last one too when it is full).
fn base64_lines(bytes: &[u8]) -> String {
    let encoded = crate::module_blob::encode_base64(bytes);
    let mut out = String::with_capacity(encoded.len() + encoded.len() / 64 * 3 + 8);
    out.push_str("#base64:");
    for chunk in encoded.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        if chunk.len() == 64 {
            out.push_str("\r\r\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::outline;

    #[test]
    fn computes_the_outlines_the_platform_stores() {
        // BSP `Задание` and ERP УХ flowcharts, point for point.
        assert_eq!(
            outline("Start", false, 380, 20, 420, 60).unwrap(),
            vec![(380, 20), (419, 20), (419, 49), (400, 59), (380, 49)]
        );
        assert_eq!(
            outline("Completion", false, 180, 460, 220, 500).unwrap(),
            vec![(200, 460), (219, 471), (219, 499), (180, 499), (180, 471)]
        );
        assert_eq!(
            outline("Condition", false, 140, 240, 260, 300).unwrap(),
            vec![
                (140, 270),
                (157, 240),
                (242, 240),
                (259, 270),
                (242, 299),
                (157, 299)
            ]
        );
        assert_eq!(
            outline("Split", false, 320, 160, 360, 180).unwrap(),
            vec![(320, 160), (358, 160), (339, 179)]
        );
        assert_eq!(
            outline("Join", false, 320, 400, 360, 420).unwrap(),
            vec![(320, 419), (358, 419), (339, 400)]
        );
        // ERP УХ templates: a group activity, a completion too low for its
        // point.
        assert_eq!(
            outline("Activity", true, 0, 0, 160, 60).unwrap(),
            vec![(0, 0), (156, 0), (156, 56), (0, 56)]
        );
        assert_eq!(
            outline("Completion", false, 0, 0, 150, 38).unwrap(),
            vec![(75, 0), (149, 19), (149, 37), (0, 37), (0, 19)]
        );
    }
}
