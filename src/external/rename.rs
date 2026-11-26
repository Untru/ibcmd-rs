//! `DataProcessor.X…` → `ExternalDataProcessor.X…` (and `Report` →
//! `ExternalReport`) in the produced XML/HTML: the pipeline names the object
//! as a configuration object, the platform as an external one. Only the bare
//! object and its `Object` type are renamed: the platform keeps
//! `DataProcessorTabularSection[Row].X.T` for external objects too (native
//! corpus: 44 occurrences, no `External…TabularSection`).
//!
//! In XML a reference is renamed only where the platform writes one: as a
//! whole element text (`<v8:Type>`, `<DefaultForm>`,
//! `<MainDataCompositionSchema>`) or a whole attribute value (the generated
//! types' `name`) -- the native corpus holds no other. A query, a string
//! value, a synonym or a comment that spells a reference is text.

use super::ExternalKind;

const VARIANTS: [&str; 2] = ["Object", ""];

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// References renamed in an XML file, user text left as written.
pub fn to_external_references(text: &str, kind: ExternalKind, name: &str) -> String {
    let internal = kind.internal_kind();
    in_reference_places(text, |value| {
        let (prefix, rest) = value.split_at(if value.starts_with("cfg:") { 4 } else { 0 });
        is_reference(rest, internal, name).then(|| format!("{prefix}External{rest}"))
    })
}

/// Whether `value` is `<internal>[Object].<name>` or a path under it.
fn is_reference(value: &str, internal: &str, name: &str) -> bool {
    VARIANTS.iter().any(|variant| {
        value
            .strip_prefix(internal)
            .and_then(|rest| rest.strip_prefix(variant))
            .and_then(|rest| rest.strip_prefix('.'))
            .and_then(|rest| rest.strip_prefix(name))
            .is_some_and(|rest| {
                rest.is_empty() || (rest.starts_with('.') && !rest.contains(char::is_whitespace))
            })
    })
}

/// `text` with every whole element text and whole attribute value that
/// `rename` answers replaced; the texts of `<v8:content>`, `<Comment>` and
/// `xs:string` values are user text and stay.
fn in_reference_places(text: &str, rename: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut pos = 0;
    // Whether the text after the last tag may be a reference.
    let mut open = false;
    while let Some(relative) = text[pos..].find('<') {
        let at = pos + relative;
        let segment = &text[pos..at];
        match rename(segment).filter(|_| open) {
            Some(renamed) => out.push_str(&renamed),
            None => out.push_str(segment),
        }
        let Some(len) = text[at..].find('>') else {
            pos = at;
            break;
        };
        let tag = &text[at..=at + len];
        out.push_str(&in_attribute_values(tag, &rename));
        let tag_name = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or("");
        open = !tag.starts_with("</")
            && !tag.starts_with("<?")
            && !tag.starts_with("<!")
            && !tag.ends_with("/>")
            && tag_name != "v8:content"
            && tag_name != "Comment"
            && !tag.contains("xsi:type=\"xs:string\"");
        pos = at + len + 1;
    }
    out.push_str(&text[pos..]);
    out
}

/// A tag with each whole attribute value that `rename` answers replaced.
fn in_attribute_values(tag: &str, rename: &impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(tag.len() + 16);
    let mut rest = tag;
    while let Some(at) = rest.find("=\"") {
        let start = at + 2;
        let Some(len) = rest[start..].find('"') else {
            break;
        };
        out.push_str(&rest[..start]);
        let value = &rest[start..start + len];
        match rename(value) {
            Some(renamed) => out.push_str(&renamed),
            None => out.push_str(value),
        }
        rest = &rest[start + len..];
    }
    out.push_str(rest);
    out
}

/// References renamed in a help page: only link targets (`href="…"`), never
/// the page's text.
pub fn to_external_html_references(text: &str, kind: ExternalKind, name: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text;
    while let Some(at) = rest.find("href=\"") {
        let start = at + "href=\"".len();
        let Some(len) = rest[start..].find('"') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&rename_all(&rest[start..start + len], kind, name));
        rest = &rest[start + len..];
    }
    out.push_str(rest);
    out
}

fn rename_all(text: &str, kind: ExternalKind, name: &str) -> String {
    let internal = kind.internal_kind();
    let mut out = text.to_owned();
    for variant in VARIANTS {
        let needle = format!("{internal}{variant}.{name}");
        let mut result = String::with_capacity(out.len() + 64);
        let mut rest = out.as_str();
        while let Some(at) = rest.find(&needle) {
            let before = rest[..at].chars().next_back();
            let after = rest[at + needle.len()..].chars().next();
            let whole = before.is_none_or(|c| !is_ident(c) && c != '.')
                && after.is_none_or(|c| !is_ident(c));
            result.push_str(&rest[..at]);
            if whole {
                result.push_str("External");
            }
            result.push_str(&needle);
            rest = &rest[at + needle.len()..];
        }
        result.push_str(rest);
        out = result;
    }
    out
}

/// The inverse of [`to_external_references`]: `ExternalDataProcessor.X…`
/// back to `DataProcessor.X…` (a tree edited in the external layout, loaded
/// through the configuration compiler), user text left as written.
pub fn to_internal_references(text: &str, kind: ExternalKind, name: &str) -> String {
    let internal = kind.internal_kind();
    in_reference_places(text, |value| {
        let (prefix, rest) = value.split_at(if value.starts_with("cfg:") { 4 } else { 0 });
        let plain = rest.strip_prefix("External")?;
        is_reference(plain, internal, name).then(|| format!("{prefix}{plain}"))
    })
}

/// The inverse of [`to_external_html_references`]: link targets only.
pub fn to_internal_html_references(text: &str, kind: ExternalKind, name: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("href=\"") {
        let start = at + "href=\"".len();
        let Some(len) = rest[start..].find('"') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&rename_back_all(&rest[start..start + len], kind, name));
        rest = &rest[start + len..];
    }
    out.push_str(rest);
    out
}

fn rename_back_all(text: &str, kind: ExternalKind, name: &str) -> String {
    let internal = kind.internal_kind();
    let mut out = text.to_owned();
    for variant in VARIANTS {
        let needle = format!("External{internal}{variant}.{name}");
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(at) = rest.find(&needle) {
            let before = rest[..at].chars().next_back();
            let after = rest[at + needle.len()..].chars().next();
            let whole = before.is_none_or(|c| !is_ident(c) && c != '.')
                && after.is_none_or(|c| !is_ident(c));
            result.push_str(&rest[..at]);
            result.push_str(if whole {
                &needle["External".len()..]
            } else {
                &needle
            });
            rest = &rest[at + needle.len()..];
        }
        result.push_str(rest);
        out = result;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<xr:GeneratedType name=\"DataProcessorObject.X\" category=\"Object\"/>\
                       <v8:Type>cfg:DataProcessorObject.X</v8:Type>\
                       <DefaultForm>DataProcessor.X.Form.Ф</DefaultForm>\
                       <v8:Type>cfg:DataProcessorTabularSectionRow.X.Т</v8:Type>";

    #[test]
    fn internal_references_invert_external_ones() {
        let t = format!("{XML}<v8:content>DataProcessor.X</v8:content>");
        let external = to_external_references(&t, ExternalKind::DataProcessor, "X");
        assert_ne!(external, t);
        assert_eq!(
            to_internal_references(&external, ExternalKind::DataProcessor, "X"),
            t
        );
    }

    #[test]
    fn renames_object_and_object_type_references() {
        assert_eq!(
            to_external_references(XML, ExternalKind::DataProcessor, "X"),
            "<xr:GeneratedType name=\"ExternalDataProcessorObject.X\" category=\"Object\"/>\
             <v8:Type>cfg:ExternalDataProcessorObject.X</v8:Type>\
             <DefaultForm>ExternalDataProcessor.X.Form.Ф</DefaultForm>\
             <v8:Type>cfg:DataProcessorTabularSectionRow.X.Т</v8:Type>"
        );
    }

    #[test]
    fn user_text_spelled_like_a_reference_is_left_alone() {
        // The platform writes an external reference only as a whole element
        // text or attribute value (native corpus: <v8:Type>, <DefaultForm>,
        // <MainDataCompositionSchema>, GeneratedType's name); a query, a
        // string value or a synonym that spells one is text.
        let xml = "<Form>DataProcessor.X.Form.Ф</Form><v8:content>см. DataProcessor.X</v8:content>\
                   <v8:content>DataProcessor.X</v8:content><Comment>DataProcessor.X</Comment>\
                   <query>ВЫБРАТЬ \"DataProcessor.X\" КАК Имя</query>\
                   <DefaultValue xsi:type=\"xs:string\">DataProcessor.X</DefaultValue>\
                   <Value title=\"DataProcessor.X и другие\"/>";
        assert_eq!(
            to_external_references(xml, ExternalKind::DataProcessor, "X"),
            "<Form>ExternalDataProcessor.X.Form.Ф</Form><v8:content>см. DataProcessor.X</v8:content>\
             <v8:content>DataProcessor.X</v8:content><Comment>DataProcessor.X</Comment>\
             <query>ВЫБРАТЬ \"DataProcessor.X\" КАК Имя</query>\
             <DefaultValue xsi:type=\"xs:string\">DataProcessor.X</DefaultValue>\
             <Value title=\"DataProcessor.X и другие\"/>"
        );
        let html = "<p>DataProcessor.X</p><a href=\"DataProcessor.X.Form.Ф/Help\">форма</a>";
        assert_eq!(
            to_external_html_references(html, ExternalKind::DataProcessor, "X"),
            "<p>DataProcessor.X</p><a href=\"ExternalDataProcessor.X.Form.Ф/Help\">форма</a>"
        );
        // The way back leaves the same text alone.
        let external = xml
            .replacen("<Form>DataProcessor", "<Form>ExternalDataProcessor", 1)
            .replace("ВЫБРАТЬ \"DataProcessor", "ВЫБРАТЬ \"ExternalDataProcessor");
        assert_eq!(
            to_internal_references(&external, ExternalKind::DataProcessor, "X"),
            xml.replace("ВЫБРАТЬ \"DataProcessor", "ВЫБРАТЬ \"ExternalDataProcessor")
        );
    }

    #[test]
    fn rename_respects_identifier_boundary() {
        let t = "<a>Report.X2</a><a>ExternalReport.X</a><a>Report.X.Template.Т</a><a>Report.X</a>";
        assert_eq!(
            to_external_references(t, ExternalKind::Report, "X"),
            "<a>Report.X2</a><a>ExternalReport.X</a><a>ExternalReport.X.Template.Т</a><a>ExternalReport.X</a>"
        );
    }
}
