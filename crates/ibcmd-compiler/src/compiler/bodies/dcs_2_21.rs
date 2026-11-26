//! What the 8.5 platform writes into a data-composition schema template
//! beyond what its 8.3.27 reading spells: a picture value of the settings
//! (`<dcscor:value xsi:type="v8ui:Picture" ref="v8ui:<name>"/>` in 2.20) is
//! written as an `xr:Picture` element naming the picture by its kind
//! (`CommonPicture.<name>`, `StdPicture.<name>`) with `LoadTransparent`
//! `true`, under a generated prefix numbered by the value element's depth.
//!
//! Evidence: the БСП 3.1.11 (8.3.27 XML) loaded by ibcmd 8.5.1.1529 into a
//! file infobase and exported by it: all 120 picture values of its templates
//! (114 common pictures, 5 standard, under `d8p1` at depth 8 and one
//! `d9p1` at depth 9) take this shape and none keeps the 2.20 one; the
//! templates of an 8.5.1.1529 export of a БСП-based `.cf` (`obf_1cv8` corpus,
//! the same five templates) do the same. The forms of the same exports keep
//! no picture value this way, so only templates are rewritten.
//!
//! Replacements use the writer's canonical line layout. Extra attributes,
//! inline neighbors and boundary text are refused instead of discarded;
//! unmodeled child content remains untouched. The compiler also proves the
//! inverse against the original source using the configuration picture index.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use quick_xml::{Reader, events::Event};

use ibcmd_form_writer::form::xml_2_21_writer::XmlEdits;

const VALUE_TAG: &str = "dcscor:value";
const PICTURE_TYPE: &str = "xsi:type=\"v8ui:Picture\"";
const REF_ATTRIBUTE: &str = " ref=\"v8ui:";
const READABLE_NS: &str = "http://v8.1c.ru/8.3/xcf/readable";
const COMMON_PICTURE: &str = "CommonPicture.";
const STD_PICTURE: &str = "StdPicture.";

fn require_known_attributes(open: &str, expected: &[(&str, &str)]) -> Result<()> {
    let source = format!("{open}>");
    let mut reader = Reader::from_str(&source);
    let event = reader.read_event()?;
    let start = match event {
        Event::Start(start) | Event::Empty(start) => start,
        _ => bail!("picture value has no opening element"),
    };
    let attributes = start
        .attributes()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if attributes.len() != expected.len()
        || attributes.iter().any(|attribute| {
            !expected.iter().any(|(name, value)| {
                attribute.key.as_ref() == name.as_bytes()
                    && attribute.value.as_ref() == value.as_bytes()
            })
        })
    {
        bail!("picture value has unmodeled attributes");
    }
    Ok(())
}

fn require_picture_lines(
    xml: &str,
    open_start: usize,
    line_end: usize,
    self_closing: bool,
) -> Result<()> {
    let indent_only = |text: &str| text.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
    let line_start = xml[..open_start].rfind('\n').map_or(0, |at| at + 1);
    // XmlEdits replaces whole lines and repeats indentation on every new line.
    // Inline neighbors or text on boundary lines are not indentation/picture data.
    if !indent_only(&xml[line_start..open_start])
        || (line_end != xml.len() && !xml[..line_end].ends_with('\n'))
    {
        bail!("picture value must occupy its own indented lines");
    }
    if !self_closing {
        let open_end = open_start + xml[open_start..].find('>').unwrap() + 1;
        let opening_tail = xml[open_end..].split('\n').next().unwrap_or_default();
        let close_start = xml[..line_end].rfind('<').unwrap();
        let close_line = xml[..close_start].rfind('\n').map_or(0, |at| at + 1);
        if !opening_tail.trim().is_empty() || !indent_only(&xml[close_line..close_start]) {
            bail!("picture value has unmodeled text on boundary lines");
        }
    }
    Ok(())
}

/// `xml` (a written schema template) with every picture value spelled the
/// 8.5 way.
#[doc(hidden)]
pub fn with_8_5_picture_values(
    xml: &str,
    object_refs: &BTreeMap<String, String>,
) -> Result<String> {
    if !xml.contains(PICTURE_TYPE) {
        return Ok(xml.to_owned());
    }
    let mut edits = XmlEdits::new(xml)?;
    let mut changed = false;
    for element in 0..edits.elements.len() {
        let item = &edits.elements[element];
        if item.tag != VALUE_TAG || !item.self_closing {
            continue;
        }
        let open = &xml[item.open_start..];
        let open = &open[..open.find('>').unwrap_or(open.len())];
        if !open.contains(PICTURE_TYPE) {
            continue;
        }
        let Some(at) = open.find(REF_ATTRIBUTE) else {
            bail!("a picture value without a picture reference: {open}");
        };
        let name = &open[at + REF_ATTRIBUTE.len()..];
        let name = &name[..name.find('"').unwrap_or(name.len())];
        if name.is_empty() {
            bail!("a picture value has an empty picture reference");
        }
        require_known_attributes(
            open,
            &[
                ("xsi:type", "v8ui:Picture"),
                ("ref", &format!("v8ui:{name}")),
            ],
        )?;
        require_picture_lines(xml, item.open_start, item.line_end, item.self_closing)?;
        let reference = picture_reference(name, object_refs);
        let mut depth = 1;
        let mut parent = item.parent;
        while let Some(index) = parent {
            depth += 1;
            parent = edits.elements[index].parent;
        }
        let prefix = format!("d{depth}p1");
        edits.replace(
            element,
            &format!(
                "<{VALUE_TAG} xmlns:{prefix}=\"{READABLE_NS}\" xsi:type=\"{prefix}:Picture\">\r\n\
                 \t<{prefix}:Ref>{reference}</{prefix}:Ref>\r\n\
                 \t<{prefix}:LoadTransparent>true</{prefix}:LoadTransparent>\r\n\
                 </{VALUE_TAG}>\r\n"
            ),
        );
        changed = true;
    }
    if !changed {
        return Ok(xml.to_owned());
    }
    edits.finish()
}

/// `CommonPicture.<name>` for a picture of the configuration, else
/// `StdPicture.<name>`: a settings picture is stored either by the uuid the
/// 2.20 writer resolved to a common or a standard picture, or by the name
/// of a platform picture (`GrayedAll`, `EventLog`, `Information` in the БСП
/// templates, which 8.5.1.1529 writes as `StdPicture.*`; `GrayedAll` is in
/// no measured uuid table).
fn picture_reference(name: &str, object_refs: &BTreeMap<String, String>) -> String {
    let common = format!("{COMMON_PICTURE}{name}");
    if object_refs.values().any(|reference| *reference == common) {
        common
    } else {
        format!("{STD_PICTURE}{name}")
    }
}

/// The inverse, for a compiled source in dialect 2.21: every picture value
/// spelled the 8.5 way back in the spelling the platform stores (`v8ui:`
/// and the bare name, for a common and a standard picture alike; the БСП
/// templates its 8.5.1.1529 import of the 8.3.27 XML stored hold 89 such
/// values in one template and no other spelling). `None` when nothing is
/// spelled that way. A value of another shape (an inline picture, `Abs`) is
/// left as written.
pub(crate) fn with_stored_picture_values(xml: &str) -> Result<Option<String>> {
    if !xml.contains(READABLE_NS) {
        return Ok(None);
    }
    let mut edits = XmlEdits::new(xml)?;
    let mut changed = false;
    let mut stored_kinds = BTreeMap::new();
    for element in 0..edits.elements.len() {
        let item = &edits.elements[element];
        if item.tag != VALUE_TAG || item.self_closing {
            continue;
        }
        let open = &xml[item.open_start..];
        let open = &open[..open.find('>').unwrap_or(open.len())];
        let Some(prefix) = readable_picture_prefix(open) else {
            continue;
        };
        require_known_attributes(
            open,
            &[
                (&format!("xmlns:{prefix}"), READABLE_NS),
                ("xsi:type", &format!("{prefix}:Picture")),
            ],
        )?;
        require_picture_lines(xml, item.open_start, item.line_end, item.self_closing)?;
        // Include content before the first child as well: inner_lines starts
        // at that child and would silently drop preceding text or comments.
        let open_end = item.open_start + xml[item.open_start..].find('>').unwrap() + 1;
        let close_start = xml[..item.line_end].rfind('<').unwrap();
        let lines: Vec<&str> = xml[open_end..close_start]
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let reference = match lines.as_slice() {
            [reference, transparent]
                if *transparent
                    == format!("<{prefix}:LoadTransparent>true</{prefix}:LoadTransparent>") =>
            {
                reference
                    .strip_prefix(&format!("<{prefix}:Ref>"))
                    .and_then(|rest| rest.strip_suffix(&format!("</{prefix}:Ref>")))
            }
            _ => None,
        };
        let Some((kind, name)) = reference.and_then(|reference| {
            [COMMON_PICTURE, STD_PICTURE]
                .into_iter()
                .find_map(|kind| reference.strip_prefix(kind).map(|name| (kind, name)))
        }) else {
            continue;
        };
        if name.is_empty() {
            bail!("a picture value has an empty picture reference");
        }
        if stored_kinds
            .insert(name.to_owned(), kind)
            .is_some_and(|previous| previous != kind)
        {
            bail!("picture kinds with the same stored name are not reversible");
        }
        edits.replace(
            element,
            &format!("<{VALUE_TAG} {PICTURE_TYPE}{REF_ATTRIBUTE}{name}\"/>\r\n"),
        );
        changed = true;
    }
    if !changed {
        return Ok(None);
    }
    edits.finish().map(Some)
}

/// The prefix of `open` (a value's start tag) when it declares the readable
/// namespace under a generated prefix and types the value as its `Picture`.
fn readable_picture_prefix(open: &str) -> Option<&str> {
    let declaration = format!("=\"{READABLE_NS}\"");
    let at = open.find(&declaration)?;
    let prefix = open[..at].rsplit_once("xmlns:")?.1;
    open.contains(&format!("xsi:type=\"{prefix}:Picture\""))
        .then_some(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_8_5_spelling_is_stored_the_platforms_way() {
        let refs = BTreeMap::from([(
            "00000000-0000-0000-0000-000000000001".to_owned(),
            "CommonPicture.Своя".to_owned(),
        )]);
        let stored = "<a>\r\n\
                      \t<b>\r\n\
                      \t\t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:Своя\"/>\r\n\
                      \t</b>\r\n\
                      \t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:AppearanceCheckBox\"/>\r\n\
                      </a>";
        let written = with_8_5_picture_values(stored, &refs).unwrap();
        assert_eq!(
            with_stored_picture_values(&written).unwrap().as_deref(),
            Some(stored)
        );
        assert_eq!(with_stored_picture_values(stored).unwrap(), None);
    }

    #[test]
    fn an_inline_picture_is_left_as_written() {
        let xml = "<a>\r\n\
                   \t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\">\r\n\
                   \t\t<d2p1:Abs>value.svg</d2p1:Abs>\r\n\
                   \t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\
                   \t</dcscor:value>\r\n\
                   </a>";
        assert_eq!(with_stored_picture_values(xml).unwrap(), None);
    }

    #[test]
    fn picture_values_name_their_kind_under_a_depth_prefix() {
        let xml = "<a>\r\n\
                   \t<b>\r\n\
                   \t\t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:Своя\"/>\r\n\
                   \t</b>\r\n\
                   \t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:AppearanceCheckBox\"/>\r\n\
                   \t<dcscor:value xsi:type=\"xs:string\">x</dcscor:value>\r\n\
                   </a>";
        let refs = BTreeMap::from([(
            "00000000-0000-0000-0000-000000000001".to_owned(),
            "CommonPicture.Своя".to_owned(),
        )]);
        assert_eq!(
            with_8_5_picture_values(xml, &refs).unwrap(),
            "<a>\r\n\
             \t<b>\r\n\
             \t\t<dcscor:value xmlns:d3p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d3p1:Picture\">\r\n\
             \t\t\t<d3p1:Ref>CommonPicture.Своя</d3p1:Ref>\r\n\
             \t\t\t<d3p1:LoadTransparent>true</d3p1:LoadTransparent>\r\n\
             \t\t</dcscor:value>\r\n\
             \t</b>\r\n\
             \t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\">\r\n\
             \t\t<d2p1:Ref>StdPicture.AppearanceCheckBox</d2p1:Ref>\r\n\
             \t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\
             \t</dcscor:value>\r\n\
             \t<dcscor:value xsi:type=\"xs:string\">x</dcscor:value>\r\n\
             </a>"
        );
    }

    #[test]
    fn picture_forward_refuses_unmodeled_value_attributes() {
        let xml = "<a>\r\n\t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:Information\" extra=\"keep\"/>\r\n</a>";
        assert!(with_8_5_picture_values(xml, &BTreeMap::new()).is_err());
    }

    #[test]
    fn picture_reverse_refuses_unmodeled_value_attributes() {
        let xml = "<a>\r\n\t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\" extra=\"keep\">\r\n\t\t<d2p1:Ref>StdPicture.Information</d2p1:Ref>\r\n\t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\t</dcscor:value>\r\n</a>";
        assert!(with_stored_picture_values(xml).is_err());
    }

    #[test]
    fn picture_forward_refuses_neighboring_inline_xml() {
        let xml = "<a><before>keep</before><dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:Information\"/><after>keep</after></a>";
        assert!(with_8_5_picture_values(xml, &BTreeMap::new()).is_err());
    }

    #[test]
    fn picture_reverse_refuses_neighboring_inline_xml() {
        let xml = "<a>\r\n\t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\">\r\n\t\t<d2p1:Ref>StdPicture.Information</d2p1:Ref>\r\n\t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\t</dcscor:value><after>keep</after></a>";
        assert!(with_stored_picture_values(xml).is_err());
    }

    #[test]
    fn picture_reverse_refuses_conflicting_kinds_for_one_stored_name() {
        let mut xml = "<a>\r\n".to_owned();
        for kind in ["CommonPicture", "StdPicture"] {
            xml.push_str(&format!("\t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\">\r\n\t\t<d2p1:Ref>{kind}.SameName</d2p1:Ref>\r\n\t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\t</dcscor:value>\r\n"));
        }
        xml.push_str("</a>");
        assert!(with_stored_picture_values(&xml).is_err());
    }

    #[test]
    fn picture_forward_refuses_an_empty_reference() {
        let xml = "<a>\r\n\t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:\"/>\r\n</a>";
        assert!(with_8_5_picture_values(xml, &BTreeMap::new()).is_err());
    }

    #[test]
    fn picture_reverse_refuses_unmodeled_text_on_boundary_lines() {
        let original = "<a>\r\n\t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\">\r\n\t\t<d2p1:Ref>StdPicture.Information</d2p1:Ref>\r\n\t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\t</dcscor:value>\r\n</a>";
        for xml in [
            original.replace("Picture\">\r\n", "Picture\">keep\r\n"),
            original.replace("\t</dcscor:value>", "\tkeep</dcscor:value>"),
        ] {
            assert!(with_stored_picture_values(&xml).is_err());
        }
    }

    #[test]
    fn picture_reverse_keeps_unmodeled_content_before_the_first_child() {
        let original = "<a>\r\n\t<dcscor:value xmlns:d2p1=\"http://v8.1c.ru/8.3/xcf/readable\" xsi:type=\"d2p1:Picture\">\r\n\t\t<d2p1:Ref>StdPicture.Information</d2p1:Ref>\r\n\t\t<d2p1:LoadTransparent>true</d2p1:LoadTransparent>\r\n\t</dcscor:value>\r\n</a>";
        for extra in ["keep", "<!-- keep -->"] {
            let xml = original.replace("Picture\">\r\n", &format!("Picture\">\r\n\t\t{extra}\r\n"));
            assert_eq!(with_stored_picture_values(&xml).unwrap(), None);
        }
    }

    #[test]
    fn picture_round_trip_keeps_neighboring_xml_bytes() {
        let xml = "<a marker=\"keep\">\r\n\t<query>SELECT &lt; &amp; keep</query>\r\n\t<!-- keep this sibling -->\r\n\t<dcscor:value xsi:type=\"v8ui:Picture\" ref=\"v8ui:Information\"/>\r\n\t<tail attr=\"keep\"> unchanged </tail>\r\n</a>";
        let written = with_8_5_picture_values(xml, &BTreeMap::new()).unwrap();
        assert_eq!(
            with_stored_picture_values(&written).unwrap().as_deref(),
            Some(xml)
        );
    }

    #[test]
    fn a_value_with_no_reference_is_refused() {
        let xml = "<a>
	<dcscor:value xsi:type=\"v8ui:Picture\"/>
</a>";
        assert!(with_8_5_picture_values(xml, &BTreeMap::new()).is_err());
    }

    #[test]
    fn a_template_without_pictures_is_left_alone() {
        let xml = "<a>\r\n\t<b/>\r\n</a>";
        assert_eq!(with_8_5_picture_values(xml, &BTreeMap::new()).unwrap(), xml);
    }
}
