//! The XDTO model cache of the configuration: a `Params` row `<uuid>.si`.
//!
//! Of the 16 `*.si` rows (derived caches the platform rewrites on every apply) one holds the XDTO model
//! of the configuration's types: raw deflate of `{2,1,{{#base64:<b64>}}}` where the base64 (64 characters a
//! line, lines separated by `CR CR LF`) encodes the model XML (`<model xmlns="http://v8.1c.ru/8.1/xdto">`,
//! UTF-8 with BOM, CRLF). A new attribute of a catalog or a document is one more `<property .../>` line in the
//! `<objectType name="CatalogObject.<Name>">` (`DocumentObject.<Name>`) element, right after the property of
//! the attribute before it (the first attribute: after the standard properties). The line has `lowerBound="0"`
//! exactly when the field is nullable (case a2: `<property name="ДемоНовыйРеквизит" type="xs:string"
//! lowerBound="0"/>`; the attributes of a flat catalog and of a document have none). Left stale,
//! serialization of the object through XDTO fails with "Свойство ... не обнаружено" while queries and
//! writes work (measured); a deleted row is rebuilt in memory by the platform.

use anyhow::{Context, Result, bail};

use crate::module_blob::{decode_base64_mime, encode_base64};
use crate::restructure::schema::TypeEntry;

const PREFIX: &str = "{2,1,\r\n{\r\n{#base64:";
const SUFFIX: &str = "}\r\n}\r\n}";
const LINE: usize = 64;
const SEPARATOR: &str = "\r\r\n";
const CORE_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/core";

/// The `<property .../>` line (without indentation and line break) of an attribute whose field has the
/// type entry `entry`. Traced on the platform: strings, numbers, booleans and dates are `xs:*`, a value
/// storage and a uuid are types of the core namespace (case h).
pub fn property_line(entry: &TypeEntry, name: &str, nullable: bool) -> Result<String> {
    let (namespace, type_name) = match (entry.tag.as_str(), entry.a) {
        ("S", _) => (None, "xs:string"),
        ("L", _) => (None, "xs:boolean"),
        ("N", _) => (None, "xs:decimal"),
        ("T", _) => (None, "xs:dateTime"),
        ("B", 16) => (Some(CORE_NAMESPACE), "d4p1:UUID"),
        ("B", 0x8000_0000) => (Some(CORE_NAMESPACE), "d4p1:ValueStorage"),
        (other, _) => bail!("no XDTO property type is known for the type tag {other}"),
    };
    let declaration = namespace
        .map(|uri| format!("xmlns:d4p1=\"{uri}\" "))
        .unwrap_or_default();
    let bound = if nullable { " lowerBound=\"0\"" } else { "" };
    Ok(format!(
        "<property {declaration}name=\"{name}\" type=\"{type_name}\"{bound}/>"
    ))
}

/// The row text (inflated, BOM optional) cut into its parts.
struct Parts {
    bom: bool,
    xml: Vec<u8>,
}

fn split(row_text: &[u8]) -> Result<Parts> {
    let (bom, text) = match row_text.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (true, rest),
        None => (false, row_text),
    };
    let text = std::str::from_utf8(text).context("the model row is not UTF-8")?;
    let body = text
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .context("the row is not an XDTO model ({2,1,{{#base64:...}}})")?;
    let stripped: String = body.split(SEPARATOR).collect();
    let xml = decode_base64_mime(&stripped).context("the model is not base64")?;
    Ok(Parts { bom, xml })
}

fn join(parts: &Parts) -> Vec<u8> {
    let encoded = encode_base64(&parts.xml);
    let mut text =
        String::with_capacity(encoded.len() + encoded.len() / LINE * SEPARATOR.len() + 64);
    text.push_str(PREFIX);
    let mut lines = encoded.as_bytes().chunks(LINE).peekable();
    while let Some(line) = lines.next() {
        text.push_str(std::str::from_utf8(line).unwrap_or_default());
        // Every full line ends with the separator, the last one too when it is full (case b2: the base64
        // of the model was a multiple of 64 characters long and the platform wrote the separator before `}`).
        if lines.peek().is_some() || line.len() == LINE {
            text.push_str(SEPARATOR);
        }
    }
    text.push_str(SUFFIX);
    let mut out = Vec::with_capacity(text.len() + 3);
    if parts.bom {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    out.extend_from_slice(text.as_bytes());
    out
}

/// Whether an inflated row is the XDTO model.
pub fn is_model(row_text: &[u8]) -> bool {
    let text = row_text
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(row_text);
    text.starts_with(PREFIX.as_bytes())
}

/// The model of a row, open for editing: the row is decoded once and encoded once, however many
/// properties are inserted (the model of a large configuration is 25 MB of XML).
pub struct Model {
    parts: Parts,
}

impl Model {
    /// Opens an inflated row.
    pub fn open(row_text: &[u8]) -> Result<Self> {
        Ok(Self {
            parts: split(row_text)?,
        })
    }

    /// Whether the model has `<objectType name="<object_type>">` (`CatalogObject.X`).
    pub fn has_object_type(&self, object_type: &str) -> bool {
        let tag = format!("<objectType name=\"{object_type}\">");
        find(&self.parts.xml, tag.as_bytes(), 0).is_some()
    }

    /// The row text again (inflated), as the platform writes it.
    pub fn to_text(&self) -> Vec<u8> {
        join(&self.parts)
    }

    /// The model XML.
    pub fn xml(&self) -> &[u8] {
        &self.parts.xml
    }
}

/// Inserts the property `line` (see [`property_line`]) of the attribute `name` into the object type
/// `object_type` (`CatalogObject.X`), right after the property `after` (the attribute before it), or,
/// without one, after the last of the `standard` properties the type has. Returns the new row text.
pub fn add_property(
    row_text: &[u8],
    object_type: &str,
    after: Option<&str>,
    standard: &[&str],
    name: &str,
    line: &str,
) -> Result<Vec<u8>> {
    let mut model = Model::open(row_text)?;
    model.add_property(object_type, after, standard, name, line)?;
    Ok(model.to_text())
}

impl Model {
    /// [`add_property`] on the open model.
    pub fn add_property(
        &mut self,
        object_type: &str,
        after: Option<&str>,
        standard: &[&str],
        name: &str,
        line: &str,
    ) -> Result<()> {
        insert_property(&mut self.parts, object_type, after, standard, name, line)
    }

    /// Removes the property line of the attribute `name` from the object type `object_type`.
    pub fn remove_property(&mut self, object_type: &str, name: &str) -> Result<()> {
        delete_property(&mut self.parts, object_type, name)
    }
}

fn delete_property(parts: &mut Parts, object_type: &str, name: &str) -> Result<()> {
    let start_tag = format!("<objectType name=\"{object_type}\">");
    let start = find(&parts.xml, start_tag.as_bytes(), 0)
        .with_context(|| format!("the model has no {start_tag}"))?;
    let end = find(&parts.xml, b"</objectType>", start).context("the object type is not closed")?;
    let named = format!(" name=\"{name}\"");
    let at = find(&parts.xml[..end], named.as_bytes(), start)
        .with_context(|| format!("{object_type} has no property {name}"))?;
    // The whole line, its terminator included.
    let line_start = parts.xml[..at]
        .windows(2)
        .rposition(|pair| pair == b"\r\n")
        .map(|position| position + 2)
        .filter(|line_start| *line_start > start)
        .context("the property line is not on a line of its own")?;
    let line_end =
        find(&parts.xml[..end], b"\r\n", at).context("the property line is not terminated")? + 2;
    let line = std::str::from_utf8(&parts.xml[line_start..line_end]).unwrap_or_default();
    if !line.trim_start().starts_with("<property ") {
        bail!("{object_type}: the line with name {name} is not a property: {line:?}");
    }
    parts.xml.drain(line_start..line_end);
    Ok(())
}

fn insert_property(
    parts: &mut Parts,
    object_type: &str,
    after: Option<&str>,
    standard: &[&str],
    name: &str,
    line: &str,
) -> Result<()> {
    let start_tag = format!("<objectType name=\"{object_type}\">");
    let start = find(&parts.xml, start_tag.as_bytes(), 0)
        .with_context(|| format!("the model has no {start_tag}"))?;
    let end = find(&parts.xml, b"</objectType>", start).context("the object type is not closed")?;
    let named = |property: &str| format!(" name=\"{property}\"");
    if find(&parts.xml[..end], named(name).as_bytes(), start).is_some() {
        bail!("{object_type} already has a property {name}");
    }
    let anchor = match after {
        Some(after) => Some(after),
        // The last standard property that the type has, by position in the text.
        None => standard
            .iter()
            .filter_map(|candidate| {
                find(&parts.xml[..end], named(candidate).as_bytes(), start)
                    .map(|at| (at, *candidate))
            })
            .max_by_key(|(at, _)| *at)
            .map(|(_, candidate)| candidate),
    };
    let anchor =
        anchor.with_context(|| format!("{object_type} has no property to put {name} after"))?;
    let at = find(&parts.xml[..end], named(anchor).as_bytes(), start)
        .with_context(|| format!("{object_type} has no property {anchor}"))?;
    let line_end = find(&parts.xml[..end], b"\r\n", at)
        .context("the property line before the new one is not terminated")?
        + 2;
    let text = format!("\t\t\t{line}\r\n");
    parts.xml.splice(line_end..line_end, text.bytes());
    Ok(())
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() || from > haystack.len() - needle.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(xml: &str) -> Vec<u8> {
        join(&Parts {
            bom: true,
            xml: xml.as_bytes().to_vec(),
        })
    }

    #[test]
    fn a_full_last_line_ends_with_the_separator_too() {
        // 48 bytes are exactly 64 base64 characters: the platform wrote the separator before the closing
        // brace (case b2); a partial last line has none.
        let full = String::from_utf8(row(&"a".repeat(48))).unwrap();
        assert!(full.ends_with("\r\r\n}\r\n}\r\n}"), "{full:?}");
        let partial = String::from_utf8(row(&"a".repeat(49))).unwrap();
        assert!(partial.ends_with("==}\r\n}\r\n}"), "{partial:?}");
        // Both read back.
        assert_eq!(split(full.as_bytes()).unwrap().xml.len(), 48);
        assert_eq!(split(partial.as_bytes()).unwrap().xml.len(), 49);
    }

    const MODEL: &str = "\u{feff}<model xmlns=\"http://v8.1c.ru/8.1/xdto\">\r\n\t<package>\r\n\t\t<objectType name=\"CatalogObject.X\">\r\n\t\t\t<property name=\"Code\" type=\"xs:string\"/>\r\n\t\t\t<property name=\"Кл\" type=\"xs:boolean\" lowerBound=\"0\"/>\r\n\t\t\t<property name=\"Таб\" type=\"d4p1:Row\" lowerBound=\"0\" upperBound=\"99999\"/>\r\n\t\t</objectType>\r\n\t</package>\r\n</model>";

    #[test]
    fn a_row_is_wrapped_like_the_platforms() {
        let text = row(MODEL);
        assert!(text.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(is_model(&text));
        let inner = std::str::from_utf8(&text[3..]).unwrap();
        assert!(inner.starts_with("{2,1,\r\n{\r\n{#base64:"));
        assert!(inner.ends_with("}\r\n}\r\n}"));
        // 64 characters a line, CR CR LF between the lines
        let body = inner
            .strip_prefix(PREFIX)
            .unwrap()
            .strip_suffix(SUFFIX)
            .unwrap();
        let lines: Vec<_> = body.split("\r\r\n").collect();
        assert!(lines.len() > 1);
        assert!(lines[..lines.len() - 1].iter().all(|line| line.len() == 64));
        assert_eq!(split(&text).unwrap().xml, MODEL.as_bytes());
    }

    #[test]
    fn a_removed_property_is_the_reverse_of_an_added_one() {
        let entry = TypeEntry::new("S", 0x8000_0032, 0, "", 0);
        let line = property_line(&entry, "Новый", true).unwrap();
        let added = add_property(
            &row(MODEL),
            "CatalogObject.X",
            Some("Кл"),
            &[],
            "Новый",
            &line,
        )
        .unwrap();
        let mut model = Model::open(&added).unwrap();
        model.remove_property("CatalogObject.X", "Новый").unwrap();
        assert_eq!(model.to_text(), row(MODEL));
        // The first, a middle and the last property go the same way.
        for name in ["Code", "Кл", "Таб"] {
            let mut model = Model::open(&row(MODEL)).unwrap();
            model.remove_property("CatalogObject.X", name).unwrap();
            let xml = String::from_utf8(model.xml().to_vec()).unwrap();
            assert!(!xml.contains(&format!(" name=\"{name}\"")), "{name}");
            assert_eq!(xml.matches("<property ").count(), 2);
        }
        // Refusals: no such type, no such property, the type's own name is no property.
        let mut model = Model::open(&row(MODEL)).unwrap();
        assert!(model.remove_property("CatalogObject.Y", "Кл").is_err());
        assert!(model.remove_property("CatalogObject.X", "Нет").is_err());
    }

    #[test]
    fn a_property_goes_after_the_property_of_the_attribute_before_it() {
        let entry = TypeEntry::new("S", 0x8000_0032, 0, "", 0);
        let line = property_line(&entry, "Новый", true).unwrap();
        assert_eq!(
            line,
            "<property name=\"Новый\" type=\"xs:string\" lowerBound=\"0\"/>"
        );
        let updated = add_property(
            &row(MODEL),
            "CatalogObject.X",
            Some("Кл"),
            &[],
            "Новый",
            &line,
        )
        .unwrap();
        let xml = String::from_utf8(split(&updated).unwrap().xml).unwrap();
        let expected = MODEL.replace(
            "\t\t\t<property name=\"Таб\"",
            "\t\t\t<property name=\"Новый\" type=\"xs:string\" lowerBound=\"0\"/>\r\n\t\t\t<property name=\"Таб\"",
        );
        assert_eq!(xml, expected);
        // Refusals: no such type, no such property, a property that is there already, no place.
        let refused = |object: &str, after: Option<&str>, name: &str| {
            add_property(&row(MODEL), object, after, &[], name, &line).is_err()
        };
        assert!(refused("CatalogObject.Y", Some("Кл"), "Н"));
        assert!(refused("CatalogObject.X", Some("Нет"), "Н"));
        assert!(refused("CatalogObject.X", Some("Кл"), "Таб"));
        assert!(refused("CatalogObject.X", None, "Н"));
    }

    #[test]
    fn the_first_attribute_goes_after_the_last_standard_property() {
        let line = property_line(&TypeEntry::new("L", 0, 0, "", 0), "Флаг", false).unwrap();
        assert_eq!(line, "<property name=\"Флаг\" type=\"xs:boolean\"/>");
        let standard = ["Ref", "Code", "Description"];
        let updated = add_property(
            &row(MODEL),
            "CatalogObject.X",
            None,
            &standard,
            "Флаг",
            &line,
        )
        .unwrap();
        let xml = String::from_utf8(split(&updated).unwrap().xml).unwrap();
        let expected = MODEL.replace(
            "\t\t\t<property name=\"Кл\"",
            "\t\t\t<property name=\"Флаг\" type=\"xs:boolean\"/>\r\n\t\t\t<property name=\"Кл\"",
        );
        assert_eq!(xml, expected);
    }

    #[test]
    fn property_lines_follow_the_type_entries() {
        let line = |tag: &str, a: u64, nullable: bool| {
            property_line(&TypeEntry::new(tag, a, 0, "", 0), "Х", nullable)
        };
        assert_eq!(
            line("S", 0, false).unwrap(),
            "<property name=\"Х\" type=\"xs:string\"/>"
        );
        assert_eq!(
            line("L", 0, false).unwrap(),
            "<property name=\"Х\" type=\"xs:boolean\"/>"
        );
        assert_eq!(
            line("N", 10, false).unwrap(),
            "<property name=\"Х\" type=\"xs:decimal\"/>"
        );
        assert_eq!(
            line("T", 0, true).unwrap(),
            "<property name=\"Х\" type=\"xs:dateTime\" lowerBound=\"0\"/>"
        );
        assert_eq!(
            line("B", 16, false).unwrap(),
            "<property xmlns:d4p1=\"http://v8.1c.ru/8.1/data/core\" name=\"Х\" type=\"d4p1:UUID\"/>"
        );
        assert_eq!(
            line("B", 0x8000_0000, false).unwrap(),
            "<property xmlns:d4p1=\"http://v8.1c.ru/8.1/data/core\" name=\"Х\" type=\"d4p1:ValueStorage\"/>"
        );
        assert!(line("B", 4, false).is_err());
        assert!(line("R", 0, false).is_err());
        assert!(!is_model(b"{0,{0}}"));
    }
}
