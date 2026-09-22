//! The 2.21 writer pass for platform 8.5 forms: what the members 8.5 appends
//! say, applied to the XML the 8.3.27 codec wrote for the down-converted body
//! (see `form_v85`).
//!
//! Every rule below is a total function over the 8.5.1.1150 BSP native tree
//! (`F:/ibcmd/lab/v85/tools/factfind.py`): for every attributable item of the
//! named kinds the member reads the listed code exactly when the platform
//! writes the listed element value. A code a rule does not list refuses the
//! form instead of writing a guessed spelling.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};

use super::form_v85::{FormV85Facts, FormV85ItemFacts, Node};

/// One element of a written `Form.xml`, located by byte offsets. The writer
/// puts every element on its own line, indented with tabs.
#[derive(Debug, Clone)]
struct XmlElement {
    tag: String,
    id: Option<String>,
    line_start: usize,
    open_start: usize,
    close_line_start: usize,
    line_end: usize,
    self_closing: bool,
    parent: Option<usize>,
    children: Vec<usize>,
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
struct XmlEdits<'a> {
    xml: &'a str,
    elements: Vec<XmlElement>,
    removed: BTreeSet<usize>,
    /// (start, end, replacement, child-order rank, sequence).
    edits: Vec<(usize, usize, String, usize, usize)>,
}

impl<'a> XmlEdits<'a> {
    fn new(xml: &'a str) -> Result<Self> {
        Ok(Self {
            xml,
            elements: scan_elements(xml)?,
            removed: BTreeSet::new(),
            edits: Vec::new(),
        })
    }

    fn indent_of(&self, element: usize) -> &'a str {
        let element = &self.elements[element];
        &self.xml[element.line_start..element.open_start]
    }

    fn direct_children(&self, parent: usize, tag: &str) -> Vec<usize> {
        self.elements[parent]
            .children
            .iter()
            .copied()
            .filter(|child| self.elements[*child].tag == tag && !self.removed.contains(child))
            .collect()
    }

    fn remove(&mut self, element: usize) {
        if self.removed.insert(element) {
            let start = self.elements[element].line_start;
            let end = self.elements[element].line_end;
            // A removal sorts after every insertion at its own start: an element
            // placed before a sibling that a later rule removes lands where
            // that sibling was, ahead of the removed text.
            let seq = self.edits.len();
            self.edits.push((start, end, String::new(), usize::MAX, seq));
        }
    }

    /// Inserts `body` (CRLF-terminated lines, relative indentation) as a child
    /// of `parent`, at the place the 2.21 child order gives `tag`.
    fn insert_child(&mut self, parent: usize, tag: &str, body: &str) -> Result<()> {
        let parent_tag = self.elements[parent].tag.clone();
        let order = super::form_v85_order::child_order(&parent_tag)
            .ok_or_else(|| anyhow!("no 2.21 child order is known for <{parent_tag}>"))?;
        let rank = |name: &str| order.iter().position(|known| *known == name);
        let new_rank =
            rank(tag).ok_or_else(|| anyhow!("<{tag}> has no known place in <{parent_tag}>"))?;
        let mut before = None;
        for &child in &self.elements[parent].children {
            if self.removed.contains(&child) {
                continue;
            }
            let child_tag = &self.elements[child].tag;
            let child_rank = rank(child_tag)
                .ok_or_else(|| anyhow!("<{child_tag}> has no known place in <{parent_tag}>"))?;
            if child_rank > new_rank {
                before = Some(child);
                break;
            }
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

    fn finish(mut self) -> Result<String> {
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
) -> Result<Option<String>> {
    let text = node.to_text();
    Ok(match object {
        FactObject::Color => {
            if text == "{3,4,{0}}" {
                return Ok(None);
            }
            let value = super::form_body::parse_form_control_color(&text, object_refs)
                .ok_or_else(|| anyhow!("<{element}>: unreadable 8.5 colour {text}"))?;
            Some(format!("<{element}>{value}</{element}>
"))
        }
        FactObject::Picture => {
            if text.starts_with("{4,0,{0},") {
                return Ok(None);
            }
            let (reference, load_transparent) =
                super::form_body::parse_form_child_item_picture_value(&text, object_refs)
                    .ok_or_else(|| anyhow!("<{element}>: unreadable 8.5 picture {text}"))?;
            Some(super::form_body::format_form_picture_element(
                element,
                Some(&reference),
                None,
                load_transparent,
                None,
                0,
            ))
        }
        FactObject::Localized => {
            if text == "{1,0}" {
                return Ok(None);
            }
            let values = super::form_body::parse_form_localized_strings(&text);
            if values.is_empty() {
                bail!("<{element}>: unreadable 8.5 localized string {text}");
            }
            Some(super::form_body::format_form_localized_section(
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

const FIELD_TAGS: &[&str] = &[
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

const GROUPING: &[(&str, Option<&str>)] = &[
    ("0", Some("Vertical")),
    ("1", Some("Horizontal")),
    ("2", Some("HorizontalIfPossible")),
    ("3", Some("AlwaysHorizontal")),
    ("4", None),
    ("5", Some("AutoScreenTypeSensitive")),
];

/// `0` false, `1` true, `2` unset.
const TRI_STATE: &[(&str, Option<&str>)] =
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
        values: &[("0", None), ("1", Some("InMainWindow"))],
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
        values: &[("0", None), ("1", Some("ByData")), ("2", Some("None"))],
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
    // The 8.3.27 reader reads `EditMode` for every field kind but this one;
    // the member is the same (1 unset, 2 `EnterOnInput`) on all 18 BSP bars.
    FactRule {
        tags: &["ProgressBarField"],
        source: FactSource::ItemMember("48", 26),
        element: "EditMode",
        replaces: &[],
        values: &[("1", None), ("2", Some("EnterOnInput"))],
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
        values: &[("0", None), ("1", Some("Activate"))],
    },
    FactRule {
        tags: &["Table"],
        source: FactSource::Item("73", 9),
        element: "RowActionsShowType",
        replaces: &[],
        values: &[("0", None), ("1", Some("DontShow")), ("2", Some("ShowOnHover"))],
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
        values: &[("0", Some("Main")), ("1", None), ("2", Some("Supplementary"))],
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
        values: &[("0", Some("Main")), ("1", None), ("2", Some("Supplementary"))],
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
    facts: &'f FormV85Facts,
    item: Option<&'f FormV85ItemFacts>,
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

fn apply_rules(
    edits: &mut XmlEdits<'_>,
    element: usize,
    facts: &FormV85Facts,
    item: Option<&FormV85ItemFacts>,
    command: Option<&[Node]>,
    object_refs: &std::collections::BTreeMap<String, String>,
) -> Result<()> {
    let tag = edits.elements[element].tag.clone();
    for rule in FACT_OBJECT_RULES {
        if !rule.tags.contains(&tag.as_str()) {
            continue;
        }
        let Some(node) = fact_node(rule.source, facts, item, command)? else {
            continue;
        };
        let body = fact_object_xml(rule.object, rule.element, node, object_refs)
            .map_err(|error| anyhow!("<{tag}> {error}"))?;
        for child in edits.direct_children(element, rule.element) {
            edits.remove(child);
        }
        if let Some(body) = body {
            edits.insert_child(element, rule.element, &body)?;
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
                match (first.and_then(Node::as_leaf), second.and_then(Node::as_leaf)) {
                    (Some(first), Some(second)) => format!("{first}|{second}"),
                    (None, None) => continue,
                    _ => bail!("<{tag}> {}: the 8.5 member pair is not scalar", rule.element),
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
            edits.insert_child(
                element,
                rule.element,
                &format!("<{0}>{value}</{0}>\r\n", rule.element),
            )?;
        }
    }
    Ok(())
}

/// Applies to a written 8.5 `Form.xml` what the appended members say.
pub(super) fn apply_v85_form_facts(
    xml: String,
    facts: &FormV85Facts,
    object_refs: &std::collections::BTreeMap<String, String>,
) -> Result<String> {
    let mut edits = XmlEdits::new(&xml)?;
    let root = edits
        .elements
        .iter()
        .position(|element| element.parent.is_none() && element.tag == "Form")
        .ok_or_else(|| anyhow!("written form XML has no <Form> root"))?;
    apply_rules(&mut edits, root, facts, None, None, object_refs)?;
    // Items by id outside the attribute, command and parameter sections,
    // whose ids number different spaces; commands by id inside theirs.
    let mut items = Vec::new();
    let mut commands = Vec::new();
    for (index, element) in edits.elements.iter().enumerate() {
        let Some(id) = element.id.clone() else {
            continue;
        };
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
            apply_rules(&mut edits, element, facts, Some(item), None, object_refs)?;
        }
    }
    for (element, id) in commands {
        if let Some(tail) = facts.commands.get(&id) {
            apply_rules(&mut edits, element, facts, None, Some(tail), object_refs)?;
        }
    }
    edits.finish()
}
