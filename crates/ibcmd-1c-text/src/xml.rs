//! The XML spelling of values read from 1C bodies: `xs:boolean` and text
//! escaping.

pub fn xml_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub fn escape_xml_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}

/// Escapes a value that stands in *element text* position, where the quote
/// character is not markup and the platform leaves it alone.
///
/// The two escaping positions are told apart by measurement, not by taste. Over
/// the eight stand corpora the platform's XML writer emits `&amp;`, `&lt;` and
/// `&gt;` (598 030 / 302 770 / 219 735 on ERP УХ 3.2.12.6 alone) and never once
/// emits `&quot;` or `&apos;` — 0 occurrences in every one of the eight trees,
/// in element text and attribute values alike — while element text carries
/// 106 631 raw `"` on ERP УХ, 4 510 on UT 11.5.27.75 and 1 473 on
/// Документооборот КОРП 3.0.21.3. A raw `>` in element text is never written
/// either, so the escape set is exactly these three characters.
///
/// This says nothing about an attribute value that *contains* a quote: the
/// corpus holds no such value, so [`escape_xml_text`] keeps its `&quot;` and
/// only positions provably outside a tag are routed here.
pub fn escape_xml_character_data(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(ch),
        }
    }
    output
}

pub fn escape_xml_element_text(value: &str) -> String {
    let value = value.replace("\r\n", "\n");
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(ch),
        }
    }
    output
}
