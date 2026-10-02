//! The 2.21 writer pass for platform 8.5 forms: what the members 8.5 appends
//! say, applied to the XML the 8.3.27 codec wrote for the down-converted body
//! (see `layout_8_5_1`).
//!
//! Every rule below is a total function over the 8.5.1.1150 BSP native tree
//! (`F:/ibcmd/lab/v85/tools/factfind.py`): for every attributable item of the
//! named kinds the member reads the listed code exactly when the platform
//! writes the listed element value. A code a rule does not list refuses the
//! form instead of writing a guessed spelling.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};

use super::layout_8_5_1::{FormFactsV8_5_1, FormItemFactsV8_5_1, Node};

/// The base form of an adopted form: a document of its own, written and passed
/// by its own facts before the form's writer encloses it
/// (`form_extension::form_adoption`), so no rule of the form's own pass reaches
/// what is under it. Its items number the same ids as the form's (fixture
/// `v85_extension/adopted_form_events`).
const BASE_FORM: &str = "BaseForm";

/// One element of a written `Form.xml`, located by byte offsets. The writer
/// puts every element on its own line, indented with tabs.
#[derive(Debug, Clone)]
pub(in crate::mssql_dump) struct XmlElement {
    pub(in crate::mssql_dump) tag: String,
    pub(in crate::mssql_dump) id: Option<String>,
    line_start: usize,
    pub(in crate::mssql_dump) open_start: usize,
    close_line_start: usize,
    pub(in crate::mssql_dump) line_end: usize,
    pub(in crate::mssql_dump) self_closing: bool,
    pub(in crate::mssql_dump) parent: Option<usize>,
    pub(in crate::mssql_dump) children: Vec<usize>,
}

fn line_start_of(xml: &str, offset: usize) -> usize {
    xml[..offset].rfind('\n').map_or(0, |newline| newline + 1)
}

fn after_line_end(xml: &str, offset: usize) -> usize {
    let rest = &xml[offset..];
    if rest.starts_with("\r\n") {
        offset + 2
    } else if rest.starts_with('\n') {
        offset + 1
    } else {
        offset
    }
}

fn scan_elements(xml: &str) -> Result<Vec<XmlElement>> {
    let bytes = xml.as_bytes();
    let mut elements: Vec<XmlElement> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut index = 0;
    while let Some(relative) = xml[index..].find('<') {
        let open = index + relative;
        match bytes.get(open + 1) {
            Some(b'?') => {
                index = xml[open..]
                    .find("?>")
                    .map(|end| open + end + 2)
                    .ok_or_else(|| anyhow!("unterminated XML declaration"))?;
            }
            Some(b'!') => {
                index = xml[open..]
                    .find("-->")
                    .map(|end| open + end + 3)
                    .ok_or_else(|| anyhow!("unterminated XML comment"))?;
            }
            Some(b'/') => {
                let close = xml[open..]
                    .find('>')
                    .map(|end| open + end)
                    .ok_or_else(|| anyhow!("unterminated closing tag"))?;
                let element = stack
                    .pop()
                    .ok_or_else(|| anyhow!("closing tag without an open element"))?;
                let tag = xml[open + 2..close].trim();
                if tag != elements[element].tag {
                    bail!("closing tag {tag} does not match {}", elements[element].tag);
                }
                elements[element].close_line_start = line_start_of(xml, open);
                elements[element].line_end = after_line_end(xml, close + 1);
                index = close + 1;
            }
            _ => {
                let mut cursor = open + 1;
                let mut quote: Option<u8> = None;
                let close = loop {
                    let Some(&byte) = bytes.get(cursor) else {
                        bail!("unterminated opening tag");
                    };
                    match quote {
                        Some(q) if byte == q => quote = None,
                        Some(_) => {}
                        None if byte == b'"' || byte == b'\'' => quote = Some(byte),
                        None if byte == b'>' => break cursor,
                        None => {}
                    }
                    cursor += 1;
                };
                let inner = &xml[open + 1..close];
                let self_closing = inner.ends_with('/');
                let tag: String = inner
                    .chars()
                    .take_while(|ch| !ch.is_whitespace() && *ch != '/' && *ch != '>')
                    .collect();
                let id = inner.find(" id=\"").and_then(|at| {
                    let value = &inner[at + 5..];
                    value.find('"').map(|end| value[..end].to_owned())
                });
                let parent = stack.last().copied();
                let line_start = line_start_of(xml, open);
                let element = elements.len();
                elements.push(XmlElement {
                    tag,
                    id,
                    line_start,
                    open_start: open,
                    close_line_start: line_start,
                    line_end: after_line_end(xml, close + 1),
                    self_closing,
                    parent,
                    children: Vec::new(),
                });
                if let Some(parent) = parent {
                    elements[parent].children.push(element);
                }
                if !self_closing {
                    stack.push(element);
                }
                index = close + 1;
            }
        }
    }
    if !stack.is_empty() {
        bail!("written form XML leaves {} element(s) open", stack.len());
    }
    Ok(elements)
}

/// Pending text edits against one written XML, applied in one pass.
pub(in crate::mssql_dump) struct XmlEdits<'a> {
    pub(in crate::mssql_dump) xml: &'a str,
    pub(in crate::mssql_dump) elements: Vec<XmlElement>,
    removed: BTreeSet<usize>,
    /// (start, end, replacement, child-order rank, sequence).
    edits: Vec<(usize, usize, String, usize, usize)>,
    /// Elements to add: (parent, tag, body), placed once every removal is
    /// known, so no element is anchored on a sibling a later rule removes.
    inserts: Vec<(usize, String, String)>,
    /// The loader's reading: an element the child order does not place goes
    /// last among its siblings instead of refusing (the 8.3.27 writer reads
    /// properties, not positions).
    lenient: bool,
}

impl<'a> XmlEdits<'a> {
    pub(in crate::mssql_dump) fn new(xml: &'a str) -> Result<Self> {
        Ok(Self {
            xml,
            elements: scan_elements(xml)?,
            removed: BTreeSet::new(),
            edits: Vec::new(),
            inserts: Vec::new(),
            lenient: false,
        })
    }

    pub(in crate::mssql_dump) fn new_lenient(xml: &'a str) -> Result<Self> {
        let mut edits = Self::new(xml)?;
        edits.lenient = true;
        Ok(edits)
    }

    /// Replaces an element with `body` (CRLF-terminated lines) at its own
    /// place and indentation.
    pub(in crate::mssql_dump) fn replace(&mut self, element: usize, body: &str) {
        if self.removed.insert(element) {
            let start = self.elements[element].line_start;
            let end = self.elements[element].line_end;
            let indent = self.indent_of(element).to_owned();
            let mut text = String::new();
            for line in body.split_inclusive("\r\n") {
                text.push_str(&indent);
                text.push_str(line);
            }
            let seq = self.edits.len();
            self.edits.push((start, end, text, usize::MAX, seq));
        }
    }

    fn indent_of(&self, element: usize) -> &'a str {
        let element = &self.elements[element];
        &self.xml[element.line_start..element.open_start]
    }

    /// The root element, `<Form>`.
    pub(in crate::mssql_dump) fn root(&self) -> Result<usize> {
        self.elements
            .iter()
            .position(|element| element.parent.is_none() && element.tag == "Form")
            .ok_or_else(|| anyhow!("written form XML has no <Form> root"))
    }

    /// The lines between the opening and closing tags of `element`: its
    /// children as written, nothing for an element written on one line.
    pub(in crate::mssql_dump) fn inner_lines(&self, element: usize) -> &'a str {
        let element = &self.elements[element];
        if element.self_closing {
            return "";
        }
        let start = element
            .children
            .first()
            .map_or(element.close_line_start, |child| {
                self.elements[*child].line_start
            });
        &self.xml[start..element.close_line_start]
    }

    /// Whether an ancestor of `element` is a `tag` element.
    pub(in crate::mssql_dump) fn within(&self, element: usize, tag: &str) -> bool {
        let mut parent = self.elements[element].parent;
        while let Some(ancestor) = parent {
            if self.elements[ancestor].tag == tag {
                return true;
            }
            parent = self.elements[ancestor].parent;
        }
        false
    }

    pub(in crate::mssql_dump) fn direct_children(&self, parent: usize, tag: &str) -> Vec<usize> {
        self.elements[parent]
            .children
            .iter()
            .copied()
            .filter(|child| self.elements[*child].tag == tag && !self.removed.contains(child))
            .collect()
    }

    pub(in crate::mssql_dump) fn remove(&mut self, element: usize) {
        if self.removed.insert(element) {
            let start = self.elements[element].line_start;
            let end = self.elements[element].line_end;
            // A removal sorts after every insertion at its own start: an element
            // placed before a sibling that a later rule removes lands where
            // that sibling was, ahead of the removed text.
            let seq = self.edits.len();
            self.edits
                .push((start, end, String::new(), usize::MAX, seq));
        }
    }

    /// Adds `body` (CRLF-terminated lines, relative indentation) as a child of
    /// `parent`, at the place the 2.21 child order gives `tag`.
    pub(in crate::mssql_dump) fn insert_child(
        &mut self,
        parent: usize,
        tag: &str,
        body: &str,
    ) -> Result<()> {
        self.inserts.push((parent, tag.to_owned(), body.to_owned()));
        Ok(())
    }

    /// Adds `body` right after `element`, at its indentation.
    fn insert_after(&mut self, element: usize, body: &str) {
        let at = self.elements[element].line_end;
        let indent = self.indent_of(element).to_owned();
        let mut text = String::new();
        for line in body.split_inclusive("\r\n") {
            text.push_str(&indent);
            text.push_str(line);
        }
        let seq = self.edits.len();
        self.edits.push((at, at, text, 0, seq));
    }

    fn place_child(&mut self, parent: usize, tag: &str, body: &str) -> Result<()> {
        let parent_tag = self.elements[parent].tag.clone();
        let order = super::xml_2_21_order::child_order(&parent_tag);
        let order = match order {
            Some(order) => order,
            None if self.lenient => &[],
            None => bail!("no 2.21 child order is known for <{parent_tag}>"),
        };
        let rank = |name: &str| order.iter().position(|known| *known == name);
        let mut before = None;
        let mut new_rank = usize::MAX - 1;
        match rank(tag) {
            Some(known) => {
                new_rank = known;
                for &child in &self.elements[parent].children {
                    if self.removed.contains(&child) {
                        continue;
                    }
                    let child_tag = &self.elements[child].tag;
                    let child_rank = match rank(child_tag) {
                        Some(child_rank) => child_rank,
                        None if self.lenient => continue,
                        None => bail!("<{child_tag}> has no known place in <{parent_tag}>"),
                    };
                    if child_rank > new_rank {
                        before = Some(child);
                        break;
                    }
                }
            }
            None if self.lenient => {}
            None => bail!("<{tag}> has no known place in <{parent_tag}>"),
        }
        let (at, indent) = match before {
            Some(child) => (
                self.elements[child].line_start,
                self.indent_of(child).to_owned(),
            ),
            None => {
                if self.elements[parent].self_closing {
                    bail!("cannot add <{tag}> to the empty <{parent_tag}>");
                }
                (
                    self.elements[parent].close_line_start,
                    format!("{}\t", self.indent_of(parent)),
                )
            }
        };
        let mut text = String::new();
        for line in body.split_inclusive("\r\n") {
            text.push_str(&indent);
            text.push_str(line);
        }
        // Several elements added before the same sibling keep the child order
        // among themselves, whatever order the rules added them in.
        let seq = self.edits.len();
        self.edits.push((at, at, text, new_rank, seq));
        Ok(())
    }

    pub(in crate::mssql_dump) fn finish(mut self) -> Result<String> {
        for (parent, tag, body) in std::mem::take(&mut self.inserts) {
            self.place_child(parent, &tag, &body)?;
        }
        self.edits
            .sort_by_key(|(start, _, _, rank, seq)| (*start, *rank, *seq));
        let mut out = String::with_capacity(self.xml.len() + 1024);
        let mut cursor = 0;
        for (start, end, text, _, _) in &self.edits {
            if *start < cursor {
                bail!("overlapping 2.21 form edits");
            }
            out.push_str(&self.xml[cursor..*start]);
            out.push_str(text);
            cursor = *end;
        }
        out.push_str(&self.xml[cursor..]);
        Ok(out)
    }
}

/// Where an appended member lives.
#[derive(Debug, Clone, Copy)]
enum FactSource {
    /// A member the root trailer appends.
    Root(usize),
    /// A member an item record appends, under the 8.5 record revision.
    Item(&'static str, usize),
    /// A member an item's property bag appends, under the 8.5 bag revision.
    Bag(&'static str, usize),
    /// Any member of an item's 8.5 property bag, where 8.5 kept a member but
    /// changed what its codes spell.
    BagMember(&'static str, usize),
    /// A member a form command appends.
    Command(usize),
    /// Two members an item record appends, read together as `a|b`.
    ItemPair(&'static str, usize, usize),
    /// A member 8.5 kept in the item record (past the optional prefix), where
    /// the 8.3.27 reader skips the item kind.
    ItemMember(&'static str, usize),
}

/// What an appended member holds, when it is not a scalar code.
#[derive(Debug, Clone, Copy)]
enum FactObject {
    /// A colour tuple; the unset `{3,4,{0}}` writes nothing.
    Color,
    /// A picture tuple; the empty `{4,0,{0},...}` writes nothing.
    Picture,
    /// A localized string; the empty `{1,0}` writes nothing.
    Localized,
}

/// A property whose appended member is a colour, picture or localized string.
struct FactObjectRule {
    tags: &'static [&'static str],
    source: FactSource,
    element: &'static str,
    object: FactObject,
}

/// Every one a total function over the 8.5.1.1150 BSP native tree, like the
/// scalar rules: the unset spelling on every item without the element, and a
/// readable value on every item with it.
const FACT_OBJECT_RULES: &[FactObjectRule] = &[
    FactObjectRule {
        tags: &["Form"],
        source: FactSource::Root(0),
        element: "CreateButtonsGroupTitle",
        object: FactObject::Localized,
    },
    FactObjectRule {
        tags: &["InputField"],
        source: FactSource::Bag("38", 0),
        element: "DropListHint",
        object: FactObject::Localized,
    },
    FactObjectRule {
        tags: &["InputField"],
        source: FactSource::Bag("38", 1),
        element: "Picture",
        object: FactObject::Picture,
    },
    FactObjectRule {
        tags: &["InputField"],
        source: FactSource::Bag("38", 2),
        element: "ChoiceButtonTitle",
        object: FactObject::Localized,
    },
    FactObjectRule {
        tags: &["PictureDecoration"],
        source: FactSource::Bag("6", 1),
        element: "PictureColor",
        object: FactObject::Color,
    },
    FactObjectRule {
        tags: &["PictureField"],
        source: FactSource::Bag("12", 1),
        element: "PictureColor",
        object: FactObject::Color,
    },
];

fn fact_object_xml(
    object: FactObject,
    element: &str,
    node: &Node,
    object_refs: &std::collections::BTreeMap<String, String>,
    payload: &mut Option<(String, Vec<u8>)>,
) -> Result<Option<String>> {
    let text = node.to_text();
    Ok(match object {
        FactObject::Color => {
            if text == "{3,4,{0}}" {
                return Ok(None);
            }
            let value = super::super::form_body::parse_form_control_color(&text, object_refs)
                .ok_or_else(|| anyhow!("<{element}>: unreadable 8.5 colour {text}"))?;
            Some(format!("<{element}>{value}</{element}>\r\n"))
        }
        FactObject::Picture => {
            if text.starts_with("{4,0,{0},") {
                return Ok(None);
            }
            if let Some((reference, load_transparent)) =
                super::super::form_body::parse_form_child_item_picture_value(&text, object_refs)
            {
                return Ok(Some(super::super::form_body::format_form_picture_element(
                    element,
                    Some(&reference),
                    None,
                    load_transparent,
                    None,
                    0,
                )));
            }
            // An inline picture: the element names the file the platform
            // writes beside the form (`Items/<item>/<element>.<ext>`).
            let (file_name, load_transparent, transparent_pixel, content) =
                super::super::form_body::parse_form_embedded_picture_payload(&text, element)
                    .ok_or_else(|| anyhow!("<{element}>: unreadable 8.5 picture {text}"))?;
            let xml = super::super::form_body::format_form_picture_element(
                element,
                None,
                Some(&file_name),
                load_transparent,
                transparent_pixel,
                0,
            );
            *payload = Some((file_name, content));
            Some(xml)
        }
        FactObject::Localized => {
            if text == "{1,0}" {
                return Ok(None);
            }
            let values = super::super::form_body::parse_form_localized_strings(&text);
            if values.is_empty() {
                bail!("<{element}>: unreadable 8.5 localized string {text}");
            }
            Some(super::super::form_body::format_form_localized_section(
                element, &values, 0,
            ))
        }
    })
}

/// One property the 2.21 writer reads from an appended member: its element,
/// the elements of the 2.20 reading it supersedes, and its full value table
/// (`None` writes no element).
struct FactRule {
    tags: &'static [&'static str],
    source: FactSource,
    element: &'static str,
    replaces: &'static [&'static str],
    values: &'static [(&'static str, Option<&'static str>)],
}

pub(in crate::mssql_dump) const FIELD_TAGS: &[&str] = &[
    "InputField",
    "LabelField",
    "CheckBoxField",
    "PictureField",
    "RadioButtonField",
    "SpreadSheetDocumentField",
    "TextDocumentField",
    "FormattedDocumentField",
    "HTMLDocumentField",
    "CalendarField",
    "GraphicalSchemaField",
    "ProgressBarField",
    "TrackBarField",
    "ChartField",
    "PlannerField",
    "PDFDocumentField",
];

pub(in crate::mssql_dump) const GROUPING: &[(&str, Option<&str>)] = &[
    ("0", Some("Vertical")),
    ("1", Some("Horizontal")),
    ("2", Some("HorizontalIfPossible")),
    ("3", Some("AlwaysHorizontal")),
    ("4", None),
    ("5", Some("AutoScreenTypeSensitive")),
];

/// `0` false, `1` true, `2` unset.
pub(in crate::mssql_dump) const TRI_STATE: &[(&str, Option<&str>)] =
    &[("0", Some("false")), ("1", Some("true")), ("2", None)];

const FACT_RULES: &[FactRule] = &[
    // Form root: appended trailer members. The 8.3.27 root slot of
    // `WindowOpeningMode` cannot tell an explicit `DontBlock` from an unset
    // mode, and 2.21 spells the owner-window lock `LockOwner`.
    FactRule {
        tags: &["Form"],
        source: FactSource::Root(4),
        element: "WindowOpeningMode",
        replaces: &[],
        values: &[
            ("0", Some("DontBlock")),
            ("1", Some("LockOwner")),
            ("2", Some("LockWholeInterface")),
            ("3", None),
        ],
    },
    FactRule {
        tags: &["Form"],
        source: FactSource::Root(5),
        element: "WindowViewMode",
        replaces: &[],
        values: &[
            ("0", None),
            ("1", Some("InMainWindow")),
            ("2", Some("InDialogWindow")),
        ],
    },
    FactRule {
        tags: &["Form"],
        source: FactSource::Root(6),
        element: "ShowCommandBar",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["Form"],
        source: FactSource::Root(7),
        element: "Group",
        replaces: &[],
        values: GROUPING,
    },
    FactRule {
        tags: &["Form"],
        source: FactSource::Root(10),
        element: "ShowTitle",
        replaces: &[],
        values: TRI_STATE,
    },
    // Fields: members the `48` record appends.
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 0),
        element: "MarkRequiredComplete",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 2),
        element: "AutoEditMode",
        replaces: &[],
        values: &[("0", None), ("1", None), ("2", None), ("3", Some("true"))],
    },
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 8),
        element: "WidthInCard",
        replaces: &[],
        values: &[("0", None), ("2", Some("Half"))],
    },
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 12),
        element: "AutoWidthInTable",
        replaces: &[],
        values: &[
            ("0", None),
            ("1", Some("ByData")),
            ("2", Some("None")),
            ("3", Some("ByDataAndTitle")),
        ],
    },
    // Members 13 and 14 read the same code on every BSP item that carries
    // either element; which element each names follows the elements' own
    // written order, not a disagreeing record.
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 13),
        element: "CellHyperlinkRepresentation",
        replaces: &[],
        values: &[("0", None), ("1", Some("Show"))],
    },
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 14),
        element: "CellHyperlinkDisplayVariant",
        replaces: &[],
        values: &[("0", None), ("1", Some("Always"))],
    },
    FactRule {
        tags: &["InputField"],
        source: FactSource::Bag("38", 3),
        element: "TextSize",
        replaces: &[],
        values: &[("0", Some("Enlarged")), ("1", None)],
    },
    FactRule {
        tags: &["LabelField"],
        source: FactSource::Bag("12", 0),
        element: "UseCopy",
        replaces: &[],
        values: &[("1", Some("true")), ("2", None)],
    },
    // 8.5 has no `Auto` check box: the kind member reads `0` where 8.3.27
    // needed its set flag, and every BSP check box with a `0` writes nothing.
    FactRule {
        tags: &["CheckBoxField"],
        source: FactSource::BagMember("13", 12),
        element: "CheckBoxType",
        replaces: &[],
        values: &[
            ("0", None),
            ("1", Some("CheckBox")),
            ("2", Some("Tumbler")),
            ("3", Some("Switcher")),
        ],
    },
    FactRule {
        tags: &["RadioButtonField"],
        source: FactSource::Bag("11", 2),
        element: "HorizontalStretch",
        replaces: &[],
        values: &[("1", Some("true")), ("2", None)],
    },
    // The 8.3.27 reader reads `EditMode` for every field kind but these; the
    // member is the same on every 8.5 BSP field of any kind (0 `Directly`, 1
    // unset, 2 `EnterOnInput`): 22 bars, 2 track bars, 1 chart.
    FactRule {
        tags: &["ProgressBarField", "TrackBarField", "ChartField"],
        source: FactSource::ItemMember("48", 26),
        element: "EditMode",
        replaces: &[],
        values: &[
            ("0", Some("Directly")),
            ("1", None),
            ("2", Some("EnterOnInput")),
        ],
    },
    // Field member 7: `0` on the one BSP check box written `false`, `1` on
    // the one input field written `true`, `2` on the 8 145 others.
    FactRule {
        tags: FIELD_TAGS,
        source: FactSource::Item("48", 7),
        element: "ShowTitleInCard",
        replaces: &[],
        values: TRI_STATE,
    },
    // The last member of the 8.3.27 button record: `2` on the one BSP button
    // written `DontChangeBehavior`; `0` and `1` on the others, which write
    // nothing.
    FactRule {
        tags: &["Button"],
        source: FactSource::ItemMember("34", 51),
        element: "OnMainServerUnavalableBehavior",
        replaces: &[],
        values: &[("0", None), ("1", None), ("2", Some("DontChangeBehavior"))],
    },
    FactRule {
        tags: &["RadioButtonField"],
        source: FactSource::Bag("11", 1),
        element: "Orientation",
        replaces: &[],
        values: &[("1", None), ("2", Some("HorizontalIfPossible"))],
    },
    // Tables: members the `73` record appends. The line and alternation
    // flags moved: 2.21 names them `...BWA` and reads them here, while the
    // 8.3.27 slots stay zero.
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 0),
        element: "HorizontalLinesBWA",
        replaces: &["HorizontalLines"],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 1),
        element: "VerticalLinesBWA",
        replaces: &["VerticalLines"],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 6),
        element: "InitialRowActivation",
        replaces: &[],
        values: &[
            ("0", None),
            ("1", Some("Activate")),
            ("2", Some("NoActivate")),
        ],
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 9),
        element: "RowActionsShowType",
        replaces: &[],
        values: &[
            ("0", None),
            ("1", Some("DontShow")),
            ("2", Some("ShowOnHover")),
        ],
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 10),
        element: "UseAlternationRowColorBWA",
        replaces: &["UseAlternationRowColor"],
        values: TRI_STATE,
    },
    // Member 11 says whether the table sets its own command bar visibility,
    // member 12 which (`2` is `auto`); unset writes nothing whatever 12 holds.
    FactRule {
        tags: &["Table"],
        source: FactSource::ItemPair("73", 11, 12),
        element: "ShowCommandBar",
        replaces: &[],
        values: &[
            ("0|0", None),
            ("0|1", None),
            ("0|2", None),
            ("1|0", Some("false")),
            ("1|1", Some("true")),
            ("1|2", Some("auto")),
        ],
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 17),
        element: "RowSelectionMode",
        replaces: &[],
        values: &[("0", Some("Cell")), ("1", Some("Row")), ("2", None)],
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 18),
        element: "AutoMaxCardHeight",
        replaces: &[],
        values: &[("0", Some("false")), ("1", None)],
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 23),
        element: "CellHyperlinksRepresentation",
        replaces: &[],
        values: &[("0", None), ("3", Some("DontShow"))],
    },
    // Buttons: members the `34` record appends.
    FactRule {
        tags: &["Button"],
        source: FactSource::Item("34", 5),
        element: "ButtonImportance",
        replaces: &[],
        values: &[
            ("0", Some("Main")),
            ("1", None),
            ("2", Some("Supplementary")),
        ],
    },
    // Groups: members their property bags append. `Group`, `Representation`
    // and `ShowTitle` moved here; the 8.3.27 slots cannot spell the new
    // `AutoScreenTypeSensitive`/`WeakSeparation` or an explicit `true`.
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::Bag("38", 0),
        element: "ShowAsCard",
        replaces: &[],
        values: &[("0", None), ("1", Some("true"))],
    },
    // 8.3.27 spelled code `1` of this member `Picture`; every BSP group that
    // carries it writes `Button` under 2.21.
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::BagMember("38", 11),
        element: "ControlRepresentation",
        replaces: &[],
        values: &[("0", None), ("1", Some("Button"))],
    },
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::Bag("38", 2),
        element: "Hyperlink",
        replaces: &[],
        values: &[("0", None), ("1", Some("true"))],
    },
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::Bag("38", 6),
        element: "Representation",
        replaces: &[],
        values: &[
            ("0", Some("None")),
            ("1", Some("StrongSeparation")),
            ("2", Some("WeakSeparation")),
            ("3", Some("NormalSeparation")),
            ("4", None),
        ],
    },
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::Bag("38", 7),
        element: "Group",
        replaces: &[],
        values: GROUPING,
    },
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::Bag("38", 9),
        element: "ScrollOnCompress",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["UsualGroup"],
        source: FactSource::Bag("38", 12),
        element: "ShowTitle",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["Page"],
        source: FactSource::Bag("22", 0),
        element: "Group",
        replaces: &[],
        values: GROUPING,
    },
    FactRule {
        tags: &["Page"],
        source: FactSource::Bag("22", 1),
        element: "ScrollOnCompress",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["Page"],
        source: FactSource::Bag("22", 3),
        element: "ShowTitle",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["ColumnGroup"],
        source: FactSource::Bag("5", 2),
        element: "ShowTitleInCard",
        replaces: &[],
        values: &[("0", Some("false")), ("2", None)],
    },
    FactRule {
        tags: &["ColumnGroup"],
        source: FactSource::Bag("5", 3),
        element: "ShowTitle",
        replaces: &[],
        values: TRI_STATE,
    },
    FactRule {
        tags: &["CommandBar"],
        source: FactSource::Bag("2", 0),
        element: "AppearanceMode",
        replaces: &[],
        values: &[("0", None), ("1", Some("CommandBar"))],
    },
    FactRule {
        tags: &["Popup"],
        source: FactSource::Bag("8", 0),
        element: "Importance",
        replaces: &[],
        values: &[
            ("0", Some("Main")),
            ("1", None),
            ("2", Some("Supplementary")),
        ],
    },
    // Form commands: members the `11` record appends.
    FactRule {
        tags: &["Command"],
        source: FactSource::Command(0),
        element: "ActionPurpose",
        replaces: &[],
        values: &[("0", None), ("1", Some("Create")), ("2", Some("Finish"))],
    },
    FactRule {
        tags: &["Command"],
        source: FactSource::Command(1),
        element: "SelectedRowsUse",
        replaces: &[],
        values: &[("0", None), ("1", Some("Use")), ("2", Some("DontUse"))],
    },
];

fn fact_node<'f>(
    source: FactSource,
    facts: &'f FormFactsV8_5_1,
    item: Option<&'f FormItemFactsV8_5_1>,
    command: Option<&'f [Node]>,
) -> Result<Option<&'f Node>> {
    Ok(match source {
        FactSource::Root(index) => facts.root_tail.get(index),
        FactSource::Item(revision, index) => match item {
            Some(item) if item.revision == revision => Some(
                item.tail
                    .get(index)
                    .ok_or_else(|| anyhow!("8.5 item record appends no member {index}"))?,
            ),
            _ => None,
        },
        FactSource::Bag(revision, index) => match item {
            Some(item) if item.bag_revision.as_deref() == Some(revision) => Some(
                item.bag_tail
                    .get(index)
                    .ok_or_else(|| anyhow!("8.5 property bag appends no member {index}"))?,
            ),
            _ => None,
        },
        FactSource::BagMember(revision, index) => match item {
            Some(item) if item.bag_revision.as_deref() == Some(revision) => Some(
                item.bag
                    .get(index)
                    .ok_or_else(|| anyhow!("8.5 property bag has no member {index}"))?,
            ),
            _ => None,
        },
        FactSource::ItemPair(..) => unreachable!("pairs are read by apply_rules"),
        FactSource::ItemMember(revision, index) => match item {
            Some(item) if item.revision == revision => Some(
                item.record
                    .get(index + item.prefix_offset)
                    .ok_or_else(|| anyhow!("8.5 item record has no member {index}"))?,
            ),
            _ => None,
        },
        FactSource::Command(index) => match command {
            Some(tail) => Some(
                tail.get(index)
                    .ok_or_else(|| anyhow!("8.5 form command appends no member {index}"))?,
            ),
            None => None,
        },
    })
}

/// The type of the enumeration value an 8.5 table keeps in member 58 when it
/// shows a settings composer's user settings.
const COMPLEX_SETTINGS_VIEW_MODE_TYPE: &str = "2eb62aaa-e6c1-48b6-a047-435354d5ae82";

/// `ComplexSettingsViewMode`: member 58 of the 8.5 table record holds the
/// value `{"#",<type>,<index>}` on exactly the three BSP tables that bind a
/// `*.UserSettings` path, each writing `Show` for index 0; every other table
/// keeps an unrelated member there and writes nothing.
fn apply_complex_settings_view_mode(
    edits: &mut XmlEdits<'_>,
    table: usize,
    item: &FormItemFactsV8_5_1,
) -> Result<()> {
    let Some(Node::List(members)) = item.record.get(58 + item.prefix_offset) else {
        return Ok(());
    };
    if members.len() != 3
        || members[0].as_leaf() != Some("\"#\"")
        || members[1].as_leaf() != Some(COMPLEX_SETTINGS_VIEW_MODE_TYPE)
    {
        return Ok(());
    }
    let value = match members[2].as_leaf() {
        Some("0") => "Show",
        other => bail!("<Table> ComplexSettingsViewMode: unknown 8.5 code {other:?}"),
    };
    for child in edits.direct_children(table, "ComplexSettingsViewMode") {
        edits.remove(child);
    }
    edits.insert_child(
        table,
        "ComplexSettingsViewMode",
        &format!("<ComplexSettingsViewMode>{value}</ComplexSettingsViewMode>\r\n"),
    )
}

/// The events 8.5 lets a usual group handle, by the identifier its event
/// record stores (8.5.1.1150 BSP: three `Click` handlers).
fn group_event_name_8_5_1(id: &str) -> Option<&'static str> {
    match id {
        "a3da1388-983a-4d28-87f2-5096d7e3a4ef" => Some("Click"),
        _ => None,
    }
}

/// A usual group's event record, the member its 8.5 property bag appends:
/// `{<count>,(<id>,"<handler>")*,1,0,(<id>,0,1)*}`, or `{0,1,0}` for none.
fn apply_group_events(edits: &mut XmlEdits<'_>, group: usize, node: &Node) -> Result<()> {
    let Node::List(members) = node else {
        bail!("<UsualGroup> events: the 8.5 member is not a tuple");
    };
    let count: usize = members
        .first()
        .and_then(Node::as_leaf)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| anyhow!("<UsualGroup> events: unreadable 8.5 count"))?;
    if members.len() != 3 + count * 5
        || members[1 + 2 * count].as_leaf() != Some("1")
        || members[2 + 2 * count].as_leaf() != Some("0")
    {
        bail!(
            "<UsualGroup> events: unexpected 8.5 record {}",
            node.to_text()
        );
    }
    if count == 0 {
        return Ok(());
    }
    let mut body = String::from("<Events>\r\n");
    for index in 0..count {
        let id = members[1 + 2 * index]
            .as_leaf()
            .ok_or_else(|| anyhow!("<UsualGroup> events: unreadable 8.5 event id"))?;
        let name = group_event_name_8_5_1(id)
            .ok_or_else(|| anyhow!("<UsualGroup> events: unknown 8.5 event {id}"))?;
        let handler = members[2 + 2 * index]
            .as_leaf()
            .and_then(|value| value.strip_prefix('"'))
            .and_then(|value| value.strip_suffix('"'))
            .ok_or_else(|| anyhow!("<UsualGroup> events: unreadable 8.5 handler"))?
            .replace("\"\"", "\"");
        body.push_str(&format!(
            "\t<Event name=\"{name}\">{}</Event>\r\n",
            super::super::escape_xml_text(&handler)
        ));
    }
    body.push_str("</Events>\r\n");
    for child in edits.direct_children(group, "Events") {
        edits.remove(child);
    }
    edits.insert_child(group, "Events", &body)
}

/// The picture 8.5 appends to a choice-list value: written after the value's
/// own `<Value>`, in document order of the values (8.5.1.1150 BSP: 17
/// pictures over 4 forms). A body whose values the written XML does not
/// match one for one refuses rather than pairing them on a guess.
fn apply_choice_value_pictures(
    edits: &mut XmlEdits<'_>,
    facts: &FormFactsV8_5_1,
    object_refs: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    let is_empty = |node: &Node| node.to_text().starts_with("{4,0,{0},");
    if facts
        .choice_value_pictures
        .iter()
        .all(|node| is_empty(node))
    {
        return Ok(());
    }
    let values = edits
        .elements
        .iter()
        .enumerate()
        .filter(|(index, element)| {
            element.tag == "xr:Value"
                && !edits.within(*index, BASE_FORM)
                && edits.xml[element.open_start..element.line_end]
                    .split('>')
                    .next()
                    .is_some_and(|open| open.contains("xsi:type=\"FormChoiceListDesTimeValue\""))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if values.len() != facts.choice_value_pictures.len() {
        bail!(
            "8.5 choice-list values carry pictures, and the written XML has {} values for {} stored",
            values.len(),
            facts.choice_value_pictures.len()
        );
    }
    for (value, picture) in values.into_iter().zip(&facts.choice_value_pictures) {
        if is_empty(picture) {
            continue;
        }
        let text = picture.to_text();
        let (reference, load_transparent) =
            super::super::form_body::parse_form_child_item_picture_value(&text, object_refs)
                .ok_or_else(|| anyhow!("unreadable 8.5 choice-list value picture {text}"))?;
        let anchor = edits
            .direct_children(value, "Value")
            .last()
            .copied()
            .ok_or_else(|| anyhow!("a written choice-list value has no <Value>"))?;
        let body = super::super::form_body::format_form_picture_element(
            "Picture",
            Some(&reference),
            None,
            load_transparent,
            None,
            0,
        );
        edits.insert_after(anchor, &body);
    }
    Ok(())
}

/// The `name` attribute of a written element's opening tag.
pub(in crate::mssql_dump) fn element_name(edits: &XmlEdits<'_>, element: usize) -> Option<String> {
    let open = edits.elements[element].open_start;
    let tag_end = edits.xml[open..].find('>')? + open;
    let inner = &edits.xml[open..tag_end];
    let at = inner.find(" name=\"")? + 7;
    let end = inner[at..].find('"')? + at;
    Some(
        inner[at..end]
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

fn apply_rules(
    edits: &mut XmlEdits<'_>,
    element: usize,
    facts: &FormFactsV8_5_1,
    item: Option<&FormItemFactsV8_5_1>,
    command: Option<&[Node]>,
    object_refs: &std::collections::BTreeMap<String, String>,
    assets: &mut Vec<super::super::FormItemAsset>,
) -> Result<()> {
    let tag = edits.elements[element].tag.clone();
    for rule in FACT_OBJECT_RULES {
        if !rule.tags.contains(&tag.as_str()) {
            continue;
        }
        let Some(node) = fact_node(rule.source, facts, item, command)? else {
            continue;
        };
        let mut payload = None;
        let body = fact_object_xml(rule.object, rule.element, node, object_refs, &mut payload)
            .map_err(|error| anyhow!("<{tag}> {error}"))?;
        if let Some((file_name, content)) = payload {
            let item_name = element_name(edits, element)
                .ok_or_else(|| anyhow!("<{tag}> with an inline picture has no name"))?;
            assets.push(super::super::FormItemAsset {
                item_name,
                file_name,
                content,
            });
        }
        for child in edits.direct_children(element, rule.element) {
            edits.remove(child);
        }
        if let Some(body) = body {
            edits.insert_child(element, rule.element, &body)?;
        }
    }
    if let Some(item) = item {
        match tag.as_str() {
            "Table" if item.revision == "73" => {
                apply_complex_settings_view_mode(edits, element, item)?
            }
            "UsualGroup" if item.bag_revision.as_deref() == Some("38") => {
                if let Some(node) = item.bag_tail.get(4) {
                    apply_group_events(edits, element, node)?;
                }
            }
            _ => {}
        }
    }
    for rule in FACT_RULES {
        if !rule.tags.contains(&tag.as_str()) {
            continue;
        }
        let code = match rule.source {
            FactSource::ItemPair(revision, first, second) => {
                let first = fact_node(FactSource::Item(revision, first), facts, item, command)?;
                let second = fact_node(FactSource::Item(revision, second), facts, item, command)?;
                match (
                    first.and_then(Node::as_leaf),
                    second.and_then(Node::as_leaf),
                ) {
                    (Some(first), Some(second)) => format!("{first}|{second}"),
                    (None, None) => continue,
                    _ => bail!(
                        "<{tag}> {}: the 8.5 member pair is not scalar",
                        rule.element
                    ),
                }
            }
            source => {
                let Some(node) = fact_node(source, facts, item, command)? else {
                    continue;
                };
                node.as_leaf()
                    .ok_or_else(|| {
                        anyhow!("<{tag}> {}: the 8.5 member is not a scalar", rule.element)
                    })?
                    .to_owned()
            }
        };
        let code = code.as_str();
        let value = rule
            .values
            .iter()
            .find(|(known, _)| *known == code)
            .map(|(_, value)| *value)
            .ok_or_else(|| anyhow!("<{tag}> {}: unknown 8.5 code {code}", rule.element))?;
        for old in std::iter::once(rule.element).chain(rule.replaces.iter().copied()) {
            for child in edits.direct_children(element, old) {
                edits.remove(child);
            }
        }
        if let Some(value) = value {
            // The one element of the table that 2.21 writes typed.
            let open = match rule.element {
                "WindowViewMode" => "WindowViewMode xsi:type=\"lf:FormWindowViewMode\"",
                element => element,
            };
            edits.insert_child(
                element,
                rule.element,
                &format!("<{open}>{value}</{0}>\r\n", rule.element),
            )?;
        }
    }
    // The appended importance member says `Main` exactly for the default button
    // (88 buttons of the БСП 8.5 ServiceDesk extension, none of them the other
    // way round), while the 8.3.27 slot that also says so is set on six of the
    // eight default buttons and left `0` on two.
    if tag == "Button"
        && fact_node(FactSource::Item("34", 5), facts, item, command)?.and_then(Node::as_leaf)
            == Some("0")
        && edits.direct_children(element, "DefaultButton").is_empty()
    {
        edits.insert_child(
            element,
            "DefaultButton",
            "<DefaultButton>true</DefaultButton>\r\n",
        )?;
    }
    Ok(())
}

/// The value of a written one-line element `<Tag ...>value</Tag>`.
pub(in crate::mssql_dump) fn simple_text<'x>(
    edits: &XmlEdits<'x>,
    element: usize,
) -> Option<&'x str> {
    let element = &edits.elements[element];
    if element.self_closing {
        return Some("");
    }
    let line = &edits.xml[element.open_start..element.line_end];
    let open_end = line.find('>')? + 1;
    let close = line[open_end..].find("</")? + open_end;
    let value = &line[open_end..close];
    (!value.contains('<')).then_some(value)
}

/// The value of the one direct child `tag` of `parent`, when it is a
/// one-line element.
pub(in crate::mssql_dump) fn child_text<'x>(
    edits: &XmlEdits<'x>,
    parent: usize,
    tag: &str,
) -> Result<Option<(usize, &'x str)>> {
    let children = edits.direct_children(parent, tag);
    match children.as_slice() {
        [] => Ok(None),
        [child] => {
            let value = simple_text(edits, *child).ok_or_else(|| {
                anyhow!(
                    "<{tag}> of <{}> is not a one-line element",
                    edits.elements[parent].tag
                )
            })?;
            Ok(Some((*child, value)))
        }
        _ => bail!("<{}> writes <{tag}> twice", edits.elements[parent].tag),
    }
}

fn add_simple(edits: &mut XmlEdits<'_>, parent: usize, tag: &str, value: &str) -> Result<()> {
    edits.insert_child(parent, tag, &format!("<{tag}>{value}</{tag}>\r\n"))
}

/// The 2.21 reading of a form body still in the 8.3.27 layout.
///
/// Platform 8.5 upgrades such a body in memory before it writes it: the
/// members 8.5 appends take their defaults, some of them derived from an
/// 8.3.27 property, and a few properties change their spelling. Each rule is
/// a total function over the 13 044 forms the 8.5.1.1150 ERP УХ export writes
/// from bodies its 8.3.27.2214 export wrote the same way, measured element by
/// element against the 8.3.27 property it follows
/// (`F:/ibcmd/lab/v85/tools/upg.py`). A value outside a rule's evidence
/// refuses the form rather than guessing a spelling.
pub(in crate::mssql_dump) fn apply_xml_2_21_upgrade_defaults(xml: String) -> Result<String> {
    let mut edits = XmlEdits::new(&xml)?;
    for index in 0..edits.elements.len() {
        if edits.within(index, BASE_FORM) {
            continue;
        }
        let tag = edits.elements[index].tag.clone();
        let is_root = edits.elements[index].parent.is_none();
        let is_item = edits.elements[index].id.is_some();
        match tag.as_str() {
            "Form" if is_root => upgrade_form_root(&mut edits, index)?,
            "Table" if is_item => upgrade_table(&mut edits, index)?,
            "Button" if is_item => upgrade_button(&mut edits, index)?,
            "Page" if is_item => {
                if child_text(&edits, index, "ScrollOnCompress")?.is_none() {
                    add_simple(&mut edits, index, "ScrollOnCompress", "false")?;
                }
                if child_text(&edits, index, "Group")?.is_none() {
                    add_simple(&mut edits, index, "Group", "Vertical")?;
                }
            }
            "UsualGroup" if is_item => upgrade_usual_group(&mut edits, index)?,
            "PictureDecoration" if is_item => upgrade_picture_color(&mut edits, index)?,
            tag if is_item && FIELD_TAGS.contains(&tag) => upgrade_field(&mut edits, index, tag)?,
            _ => {}
        }
    }
    edits.finish()
}

fn upgrade_form_root(edits: &mut XmlEdits<'_>, form: usize) -> Result<()> {
    if child_text(edits, form, "Group")?.is_none() {
        add_simple(edits, form, "Group", "Vertical")?;
    }
    match child_text(edits, form, "WindowOpeningMode")? {
        None => add_simple(edits, form, "WindowOpeningMode", "DontBlock")?,
        Some((child, "LockOwnerWindow")) => {
            edits.remove(child);
            add_simple(edits, form, "WindowOpeningMode", "LockOwner")?;
        }
        Some((_, "LockWholeInterface")) => {}
        Some((_, value)) => bail!("<Form> WindowOpeningMode {value} has no 2.21 upgrade"),
    }
    if child_text(edits, form, "ShowCommandBar")?.is_none() {
        match child_text(edits, form, "CommandBarLocation")?.map(|(_, value)| value) {
            None => {}
            Some("Top" | "Bottom") => add_simple(edits, form, "ShowCommandBar", "true")?,
            Some("None") => add_simple(edits, form, "ShowCommandBar", "false")?,
            Some(value) => bail!("<Form> CommandBarLocation {value} has no 2.21 upgrade"),
        }
    }
    Ok(())
}

fn upgrade_table(edits: &mut XmlEdits<'_>, table: usize) -> Result<()> {
    if child_text(edits, table, "RowSelectionMode")?.is_none() {
        add_simple(edits, table, "RowSelectionMode", "Cell")?;
    }
    match child_text(edits, table, "UseAlternationRowColor")? {
        None => add_simple(edits, table, "UseAlternationRowColorBWA", "false")?,
        Some((child, "true")) => edits.remove(child),
        Some((_, value)) => bail!("<Table> UseAlternationRowColor {value} has no 2.21 upgrade"),
    }
    for (old, new) in [
        ("HorizontalLines", "HorizontalLinesBWA"),
        ("VerticalLines", "VerticalLinesBWA"),
    ] {
        if let Some((child, value)) = child_text(edits, table, old)? {
            let value = value.to_owned();
            edits.remove(child);
            add_simple(edits, table, new, &value)?;
        }
    }
    let user_settings = child_text(edits, table, "DataPath")?
        .is_some_and(|(_, path)| path.ends_with(".UserSettings"));
    if user_settings && child_text(edits, table, "ComplexSettingsViewMode")?.is_none() {
        add_simple(edits, table, "ComplexSettingsViewMode", "Show")?;
    }
    Ok(())
}

fn upgrade_button(edits: &mut XmlEdits<'_>, button: usize) -> Result<()> {
    if child_text(edits, button, "ButtonImportance")?.is_none() {
        let default = child_text(edits, button, "DefaultButton")?.map(|(_, value)| value);
        let shape = child_text(edits, button, "ShapeRepresentation")?.map(|(_, value)| value);
        match (default, shape) {
            (Some("true"), _) => add_simple(edits, button, "ButtonImportance", "Main")?,
            (None, Some("None")) => add_simple(edits, button, "ButtonImportance", "Supplementary")?,
            (None, _) => {}
            (Some(value), _) => bail!("<Button> DefaultButton {value} has no 2.21 upgrade"),
        }
    }
    if let Some((child, "auto")) = child_text(edits, button, "BackColor")? {
        edits.remove(child);
    }
    Ok(())
}

/// An 8.5 button record whose `ButtonImportance` member holds code `1` (no
/// importance of its own) on a default button: 8.5.1.1529 writes `Main`, as
/// it does for the 8.3.27 default button [`upgrade_button`] reads (fixture
/// `v85_extension/v85_form`: a button saved by 8.5 with `DefaultButton`
/// `true` and code `1`). 8.5.1.1150 writes nothing there: three default
/// buttons of the БСП 8.5 configuration (`DataProcessors/ИнформационныйЦентр`,
/// `Reports/ИсторияРазмераПриложения`) carry code `1` and no importance in its
/// own dump. The rule therefore applies to the builds whose registry profile
/// declares `platform.form.default_button_importance`.
fn default_button_importance(
    edits: &mut XmlEdits<'_>,
    button: usize,
    item: &FormItemFactsV8_5_1,
) -> Result<()> {
    let own_importance = item.tail.get(5).and_then(Node::as_leaf);
    if edits.elements[button].tag != "Button"
        || item.revision != "34"
        || own_importance != Some("1")
    {
        return Ok(());
    }
    if child_text(edits, button, "ButtonImportance")?.is_none()
        && child_text(edits, button, "DefaultButton")?.is_some_and(|(_, value)| value == "true")
    {
        add_simple(edits, button, "ButtonImportance", "Main")?;
    }
    Ok(())
}

fn upgrade_usual_group(edits: &mut XmlEdits<'_>, group: usize) -> Result<()> {
    if child_text(edits, group, "Representation")?.is_none() {
        add_simple(edits, group, "Representation", "WeakSeparation")?;
    }
    if child_text(edits, group, "Group")?.is_none() {
        add_simple(edits, group, "Group", "HorizontalIfPossible")?;
    }
    match child_text(edits, group, "ControlRepresentation")? {
        None => {}
        Some((child, "Picture")) => {
            edits.remove(child);
            add_simple(edits, group, "ControlRepresentation", "Button")?;
        }
        Some((_, value)) => bail!("<UsualGroup> ControlRepresentation {value} has no 2.21 upgrade"),
    }
    Ok(())
}

/// 2.21 colours a picture with its own `PictureColor`, which an upgraded
/// body takes from the item's text colour.
fn upgrade_picture_color(edits: &mut XmlEdits<'_>, item: usize) -> Result<()> {
    if child_text(edits, item, "PictureColor")?.is_none()
        && let Some((_, color)) = child_text(edits, item, "TextColor")?
    {
        let color = color.to_owned();
        add_simple(edits, item, "PictureColor", &color)?;
    }
    Ok(())
}

fn upgrade_field(edits: &mut XmlEdits<'_>, field: usize, tag: &str) -> Result<()> {
    if child_text(edits, field, "AutoEditMode")?.is_none() {
        match child_text(edits, field, "EditMode")?.map(|(_, value)| value) {
            Some("EnterOnInput") => add_simple(edits, field, "AutoEditMode", "true")?,
            None | Some("Directly") => {}
            Some(value) => bail!("<{tag}> EditMode {value} has no 2.21 upgrade"),
        }
    }
    if tag == "InputField" && child_text(edits, field, "MarkRequiredComplete")?.is_none() {
        match child_text(edits, field, "AutoMarkIncomplete")?.map(|(_, value)| value) {
            None => {}
            Some(value @ ("true" | "false")) => {
                let value = value.to_owned();
                add_simple(edits, field, "MarkRequiredComplete", &value)?;
            }
            Some(value) => bail!("<{tag}> AutoMarkIncomplete {value} has no 2.21 upgrade"),
        }
    }
    match child_text(edits, field, "CellHyperlink")?.map(|(_, value)| value) {
        None => {}
        Some("true") => {
            if child_text(edits, field, "CellHyperlinkRepresentation")?.is_none() {
                add_simple(edits, field, "CellHyperlinkRepresentation", "Show")?;
            }
            if child_text(edits, field, "CellHyperlinkDisplayVariant")?.is_none() {
                add_simple(edits, field, "CellHyperlinkDisplayVariant", "Always")?;
            }
        }
        Some(value) => bail!("<{tag}> CellHyperlink {value} has no 2.21 upgrade"),
    }
    if tag == "CheckBoxField"
        && let Some((child, "Auto")) = child_text(edits, field, "CheckBoxType")?
    {
        edits.remove(child);
    }
    if tag == "PictureField" {
        upgrade_picture_color(edits, field)?;
    }
    Ok(())
}

/// Applies to a written 8.5 `Form.xml` what the appended members say, and
/// returns the inline pictures those members carry (files beside the form).
pub(in crate::mssql_dump) fn apply_form_facts_8_5_1(
    xml: String,
    facts: &FormFactsV8_5_1,
    object_refs: &std::collections::BTreeMap<String, String>,
) -> Result<(String, Vec<super::super::FormItemAsset>)> {
    let mut assets = Vec::new();
    let mut edits = XmlEdits::new(&xml)?;
    let root = edits.root()?;
    apply_rules(
        &mut edits,
        root,
        facts,
        None,
        None,
        object_refs,
        &mut assets,
    )?;
    apply_choice_value_pictures(&mut edits, facts, object_refs)?;
    match facts.root_scale.as_deref() {
        None | Some("100") => {}
        Some(scale) => {
            for child in edits.direct_children(root, "Scale") {
                edits.remove(child);
            }
            edits.insert_child(root, "Scale", &format!("<Scale>{scale}</Scale>\r\n"))?;
        }
    }
    // Items by id outside the attribute, command and parameter sections,
    // whose ids number different spaces; commands by id inside theirs.
    let mut items = Vec::new();
    let mut commands = Vec::new();
    for (index, element) in edits.elements.iter().enumerate() {
        let Some(id) = element.id.clone() else {
            continue;
        };
        if edits.within(index, BASE_FORM) {
            continue;
        }
        let mut section = None;
        let mut parent = element.parent;
        while let Some(ancestor) = parent {
            let tag = edits.elements[ancestor].tag.as_str();
            if matches!(
                tag,
                "Attributes" | "Commands" | "Parameters" | "CommandInterface"
            ) {
                section = Some(tag);
                break;
            }
            parent = edits.elements[ancestor].parent;
        }
        match section {
            None => items.push((index, id)),
            Some("Commands") if element.tag == "Command" => commands.push((index, id)),
            Some(_) => {}
        }
    }
    for (element, id) in items {
        if let Some(item) = facts.items.get(&id) {
            apply_rules(
                &mut edits,
                element,
                facts,
                Some(item),
                None,
                object_refs,
                &mut assets,
            )?;
            if crate::platform::export_writes_default_button_importance() {
                default_button_importance(&mut edits, element, item)?;
            }
        }
    }
    for (element, id) in commands {
        if let Some(tail) = facts.commands.get(&id) {
            apply_rules(
                &mut edits,
                element,
                facts,
                None,
                Some(tail),
                object_refs,
                &mut assets,
            )?;
        }
    }
    Ok((edits.finish()?, assets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn button(name: &str, id: u32, extra: &str) -> String {
        format!(
            "\t\t<Button name=\"{name}\" id=\"{id}\">\r\n\
\t\t\t<Type>UsualButton</Type>\r\n{extra}\
\t\t\t<CommandName>Form.Command.{name}</CommandName>\r\n\
\t\t</Button>\r\n"
        )
    }

    fn form(buttons: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" version=\"2.21\">\r\n\
\t<ChildItems>\r\n{buttons}\t</ChildItems>\r\n</Form>"
        )
    }

    /// A button record of revision 34 whose appended importance member is `importance`.
    fn button_facts(importance: &str) -> FormItemFactsV8_5_1 {
        FormItemFactsV8_5_1 {
            revision: "34".to_owned(),
            // Members 0..=5 appended; member 5 is the importance.
            tail: (0..6)
                .map(|index| Node::Leaf(if index == 5 { importance } else { "0" }.to_owned()))
                .collect(),
            // The record ahead of them; its last member (51) is the
            // main-server-unavailable behavior.
            record: vec![Node::Leaf("0".to_owned()); 60],
            ..FormItemFactsV8_5_1::default()
        }
    }

    /// The appended importance member of a button says `Main` exactly for its
    /// default button (88 of 88 on the БСП 8.5 extension ServiceDesk); the 8.3.27
    /// slot that also says so is set on six of the eight default buttons.
    #[test]
    fn a_main_button_is_the_default_button() {
        let xml = form(
            &[
                button("Create", 20, ""),
                button("Other", 21, ""),
                button("Help", 22, ""),
                // The 8.3.27 slot already said so: written once, not twice.
                button("Known", 23, "\t\t\t<DefaultButton>true</DefaultButton>\r\n"),
            ]
            .concat(),
        );
        let facts = FormFactsV8_5_1 {
            items: BTreeMap::from([
                ("20".to_owned(), button_facts("0")),
                ("21".to_owned(), button_facts("1")),
                ("22".to_owned(), button_facts("2")),
                ("23".to_owned(), button_facts("0")),
            ]),
            ..FormFactsV8_5_1::default()
        };
        let (written, assets) = apply_form_facts_8_5_1(xml, &facts, &BTreeMap::new()).unwrap();
        assert!(assets.is_empty());
        let expected = form(
            &[
                button(
                    "Create",
                    20,
                    "\t\t\t<DefaultButton>true</DefaultButton>\r\n",
                )
                .replace(
                    "\t\t</Button>",
                    "\t\t\t<ButtonImportance>Main</ButtonImportance>\r\n\t\t</Button>",
                ),
                button("Other", 21, ""),
                button("Help", 22, "").replace(
                    "\t\t</Button>",
                    "\t\t\t<ButtonImportance>Supplementary</ButtonImportance>\r\n\t\t</Button>",
                ),
                button("Known", 23, "\t\t\t<DefaultButton>true</DefaultButton>\r\n").replace(
                    "\t\t</Button>",
                    "\t\t\t<ButtonImportance>Main</ButtonImportance>\r\n\t\t</Button>",
                ),
            ]
            .concat(),
        );
        assert_eq!(written, expected);
    }
}
