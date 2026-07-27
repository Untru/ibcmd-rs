//! Canonical metadata child-object composition.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

const EMPTY_CHILD_OBJECTS: &str = "\t\t<ChildObjects/>\r\n";
const OPEN_CHILD_OBJECTS: &str = "\t\t<ChildObjects>\r\n";
const CLOSE_CHILD_OBJECTS: &str = "\t\t</ChildObjects>\r\n";

/// Failure to append verified CCT template children atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CctTemplateChildrenError {
    /// The caller supplied neither the exact empty fragment nor an exact
    /// already-opened child collection ending in the canonical close tag.
    InvalidExistingFragment,
    /// A template name contains a character forbidden by XML 1.0.
    InvalidTemplateName {
        /// Zero-based template index.
        index: usize,
    },
}

impl Display for CctTemplateChildrenError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExistingFragment => {
                formatter.write_str("invalid canonical CCT ChildObjects fragment")
            }
            Self::InvalidTemplateName { index } => {
                write!(
                    formatter,
                    "invalid XML 1.0 character in CCT template {index}"
                )
            }
        }
    }
}

impl Error for CctTemplateChildrenError {}

/// Appends owned CCT templates after all metadata children and forms.
///
/// Native/EDT ordering places templates last even though the physical owner
/// collection declares them before tabular sections and forms.
pub fn append_cct_template_children(
    existing: String,
    templates: &[String],
) -> Result<String, CctTemplateChildrenError> {
    let empty_collection = existing == EMPTY_CHILD_OBJECTS;
    let populated_collection =
        existing.starts_with(OPEN_CHILD_OBJECTS) && existing.ends_with(CLOSE_CHILD_OBJECTS);
    if !empty_collection && !populated_collection {
        return Err(CctTemplateChildrenError::InvalidExistingFragment);
    }
    if templates.is_empty() {
        return Ok(existing);
    }
    for (index, name) in templates.iter().enumerate() {
        if name.chars().any(|character| !is_xml_1_0_char(character)) {
            return Err(CctTemplateChildrenError::InvalidTemplateName { index });
        }
    }

    let mut output = if empty_collection {
        OPEN_CHILD_OBJECTS.to_string()
    } else {
        existing
            .strip_suffix(CLOSE_CHILD_OBJECTS)
            .ok_or(CctTemplateChildrenError::InvalidExistingFragment)?
            .to_string()
    };
    for name in templates {
        output.push_str("\t\t\t<Template>");
        output.push_str(&escape_element_text(name));
        output.push_str("</Template>\r\n");
    }
    output.push_str(CLOSE_CHILD_OBJECTS);
    Ok(output)
}

fn escape_element_text(value: &str) -> String {
    let value = value.replace("\r\n", "\n");
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
    output
}

fn is_xml_1_0_char(character: char) -> bool {
    matches!(
        character,
        '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_empty_shape_without_templates() {
        assert_eq!(
            append_cct_template_children(EMPTY_CHILD_OBJECTS.to_string(), &[]).unwrap(),
            EMPTY_CHILD_OBJECTS
        );
    }

    #[test]
    fn appends_templates_after_existing_forms_and_escapes_text() {
        let existing = concat!(
            "\t\t<ChildObjects>\r\n",
            "\t\t\t<Form>Existing</Form>\r\n",
            "\t\t</ChildObjects>\r\n"
        );
        let output =
            append_cct_template_children(existing.to_string(), &["Template<&>\"'".to_string()])
                .unwrap();
        assert_eq!(
            output,
            concat!(
                "\t\t<ChildObjects>\r\n",
                "\t\t\t<Form>Existing</Form>\r\n",
                "\t\t\t<Template>Template&lt;&amp;&gt;\"'</Template>\r\n",
                "\t\t</ChildObjects>\r\n"
            )
        );
    }

    #[test]
    fn opens_empty_collection_and_rejects_invalid_input_atomically() {
        assert_eq!(
            append_cct_template_children(EMPTY_CHILD_OBJECTS.to_string(), &["Only".to_string()])
                .unwrap(),
            concat!(
                "\t\t<ChildObjects>\r\n",
                "\t\t\t<Template>Only</Template>\r\n",
                "\t\t</ChildObjects>\r\n"
            )
        );
        assert_eq!(
            append_cct_template_children("wrong".to_string(), &["Only".to_string()]),
            Err(CctTemplateChildrenError::InvalidExistingFragment)
        );
        assert_eq!(
            append_cct_template_children("wrong".to_string(), &[]),
            Err(CctTemplateChildrenError::InvalidExistingFragment)
        );
        assert_eq!(
            append_cct_template_children(
                "wrong\t\t</ChildObjects>\r\n".to_string(),
                &["Only".to_string()]
            ),
            Err(CctTemplateChildrenError::InvalidExistingFragment)
        );
        assert_eq!(
            append_cct_template_children(
                EMPTY_CHILD_OBJECTS.to_string(),
                &["bad\u{0}value".to_string()]
            ),
            Err(CctTemplateChildrenError::InvalidTemplateName { index: 0 })
        );
    }
}
